use async_trait::async_trait;
use executor_trait::{Executor, Task};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::runtime::Handle;

/// Dummy object implementing executor-trait common interfaces on top of tokio
#[derive(Debug, Default, Clone)]
pub struct Tokio(Option<Handle>);

impl Tokio {
    pub fn with_handle(mut self, handle: Handle) -> Self {
        self.0 = Some(handle);
        self
    }

    pub fn current() -> Self {
        Self::default().with_handle(Handle::current())
    }
}

struct TTask(tokio::task::JoinHandle<()>);

#[async_trait]
impl Executor for Tokio {
    fn block_on(&self, f: Pin<Box<dyn Future<Output = ()>>>) {
        // FIXME: use the current runtime once Handle::block_on gets available
        tokio::runtime::Runtime::new()
            .expect("failed to create runtime")
            .block_on(f);
    }

    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Box<dyn Task> {
        Box::new(TTask(if let Some(handle) = self.0.as_ref() {
            handle.spawn(f)
        } else {
            tokio::task::spawn(f)
        }))
    }

    fn spawn_local(&self, f: Pin<Box<dyn Future<Output = ()>>>) -> Box<dyn Task> {
        Box::new(TTask(tokio::task::spawn_local(f)))
    }

    async fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) {
        // FIXME: better handling of failure
        if let Some(handle) = self.0.as_ref() {
            handle.spawn_blocking(f).await
        } else {
            tokio::task::spawn_blocking(f).await
        }
        .expect("blocking task failed");
    }
}

#[async_trait(?Send)]
impl Task for TTask {
    async fn cancel(self: Box<Self>) -> Option<()> {
        self.0.abort();
        self.0.await.ok()
    }
}

impl Future for TTask {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => Poll::Ready(res.expect("task has been canceled")),
        }
    }
}
