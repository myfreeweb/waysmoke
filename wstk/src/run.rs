use async_trait::async_trait;

#[async_trait(?Send)]
pub trait Runnable {
    /// Run one iteration of the event loop for this object
    async fn run(&mut self) -> bool;
}
