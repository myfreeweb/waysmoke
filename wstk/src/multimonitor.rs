use crate::{
    run::*,
    surfaces::{output, wl_output, Env, Environment},
};
use async_trait::async_trait;
use futures::{channel::mpsc, Future, FutureExt, StreamExt};

pub struct MultiMonitor<'a, T, F> {
    _osl: output::OutputStatusListener,
    rx: mpsc::UnboundedReceiver<wl_output::WlOutput>,
    instances: Vec<T>,
    mk: Box<dyn 'a + Fn(wl_output::WlOutput) -> F>,
}

impl<'a, T, F> MultiMonitor<'a, T, F>
where
    T: Runnable,
    F: Future<Output = T>,
{
    pub async fn new(
        mk: Box<dyn 'a + Fn(wl_output::WlOutput) -> F>,
        env: &'a Environment<Env>,
    ) -> MultiMonitor<'a, T, F> {
        let (tx, rx) = mpsc::unbounded();
        let mut instances = Vec::new();

        let _osl = env.listen_for_outputs(move |output, info, _| {
            if info.obsolete {
                return;
            }
            if let Err(e) = tx.unbounded_send(output) {
                if !e.is_disconnected() {
                    panic!("Unexpected send error {:?}", e)
                }
            }
        });

        for output in env.get_all_outputs() {
            instances.push(mk(output).await);
        }

        MultiMonitor {
            _osl,
            rx,
            instances,
            mk,
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
            inst_res = run_instances.select_next_some() => {
                drop(run_instances);
                let (cont, idx) = inst_res;
                if !cont {
                    this.instances.remove(idx);
                    if this.instances.is_empty() {
                        return false
                    }
                }
            },
            output = this.rx.select_next_some() => {
                drop(run_instances);
                this.instances.push(
                    (this.mk)(output).await,
                );
            }
        }
        true
    }
}
