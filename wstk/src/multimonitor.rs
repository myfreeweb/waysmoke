use crate::{
    run::*,
    surfaces::{output, wl_output, Env, Environment},
};
use async_trait::async_trait;
use futures::{channel::mpsc, future::LocalBoxFuture, FutureExt, StreamExt};

pub struct MultiMonitor<'a, T> {
    _osl: output::OutputStatusListener,
    rx: mpsc::UnboundedReceiver<(wl_output::WlOutput, output::OutputInfo)>,
    instances: Vec<T>,
    mk: Box<dyn 'a + Fn(wl_output::WlOutput, output::OutputInfo) -> LocalBoxFuture<'a, T>>,
}

impl<'a, T> MultiMonitor<'a, T>
where
    T: Runnable,
{
    pub async fn new(
        mk: Box<dyn 'a + Fn(wl_output::WlOutput, output::OutputInfo) -> LocalBoxFuture<'a, T>>,
        env: &'a Environment<Env>,
    ) -> MultiMonitor<'a, T> {
        let (tx, rx) = mpsc::unbounded();
        let mut instances = Vec::new();

        let _osl = env.listen_for_outputs(move |output, info, _| {
            if info.obsolete {
                return;
            }
            if let Err(e) = tx.unbounded_send((output, info.clone())) {
                if !e.is_disconnected() {
                    panic!("Unexpected send error {:?}", e)
                }
            }
        });

        for output in env.get_all_outputs() {
            if let Some(info) = output::with_output_info(&output, Clone::clone) {
                instances.push(mk(output, info).await);
            } else {
                eprintln!("Could not get output info?");
            }
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
impl<'a, T> Runnable for MultiMonitor<'a, T>
where
    T: Runnable,
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
            (output, info) = this.rx.select_next_some() => {
                drop(run_instances);
                this.instances.push(
                    (this.mk)(output, info).await,
                );
            }
        }
        true
    }
}
