use crate::{
    bus,
    run::*,
    surfaces::{wl_output, Env, Environment},
};
use async_trait::async_trait;
use futures::{stream, Future, FutureExt, StreamExt};

pub struct MultiMonitor<'a, T, F> {
    instances: Vec<T>,
    mk: Box<dyn 'a + Fn(wl_output::WlOutput) -> F>,
    recv: stream::Fuse<bus::Subscriber<wl_output::WlOutput>>,
}

impl<'a, T, F> MultiMonitor<'a, T, F>
where
    T: Runnable,
    F: Future<Output = T>,
{
    pub async fn new(
        mk: Box<dyn 'a + Fn(wl_output::WlOutput) -> F>,
        env: &'a Environment<Env>,
        recv: bus::Subscriber<wl_output::WlOutput>,
    ) -> MultiMonitor<'a, T, F> {
        let mut instances = Vec::new();
        for output in env.get_all_outputs() {
            instances.push(mk(output).await);
        }

        MultiMonitor {
            instances,
            mk,
            recv: recv.fuse(),
        }
    }
}

#[async_trait(?Send)]
impl<'a, T, F> Runnable for MultiMonitor<'a, T, F>
where
    T: Runnable,
    F: Future<Output = T>,
{
    async fn run(&mut self) -> bool {
        let this = self; // argh macro weirdness
        let mut run_instances = this
            .instances
            .iter_mut()
            .enumerate()
            .map(|(i, x)| x.run().map(move |res| (res, i)))
            .collect::<futures::stream::FuturesUnordered<_>>();
        futures::select! {
            inst_res = run_instances.next() => {
                drop(run_instances);
                let (cont, idx) = inst_res.unwrap();
                if !cont {
                    this.instances.remove(idx);
                    if this.instances.is_empty() {
                        return false
                    }
                }
            },
            output = this.recv.next() => {
                drop(run_instances);
                this.instances.push(
                    (this.mk)(
                        (*output.unwrap()).clone(),
                    )
                    .await,
                );
            }
        }
        true
    }
}
