use std::{future::Future, pin::Pin};

use pin_project::{pin_project, pinned_drop};

#[pin_project(PinnedDrop)]
pub struct DroppableFuture<F, D>
where
    F: Future,
    D: Fn(),
{
    #[pin]
    future: F,
    on_drop: D,
}

impl<F, D> DroppableFuture<F, D>
where
    F: Future,
    D: Fn(),
{
    pub fn new(future: F, on_drop: D) -> Self {
        Self { future, on_drop }
    }
}

impl<F, D> Future for DroppableFuture<F, D>
where
    F: Future,
    D: Fn(),
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx)
    }
}

#[pinned_drop]
impl<F, D> PinnedDrop for DroppableFuture<F, D>
where
    F: Future,
    D: Fn(),
{
    fn drop(self: Pin<&mut Self>) {
        (self.on_drop)();
    }
}
