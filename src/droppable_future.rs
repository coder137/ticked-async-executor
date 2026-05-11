use std::{future::Future, pin::Pin};

use pin_project::{pin_project, pinned_drop};

#[pin_project(PinnedDrop)]
pub struct DroppableFuture<F, D>
where
    F: Future,
    D: FnOnce(),
{
    #[pin]
    future: F,
    on_drop: Option<D>,
}

impl<F, D> DroppableFuture<F, D>
where
    F: Future,
    D: FnOnce(),
{
    pub fn new(future: F, on_drop: D) -> Self {
        Self {
            future,
            on_drop: Some(on_drop),
        }
    }
}

impl<F, D> Future for DroppableFuture<F, D>
where
    F: Future,
    D: FnOnce(),
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
    D: FnOnce(),
{
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();
        if let Some(on_drop) = this.on_drop.take() {
            (on_drop)();
        }
    }
}
