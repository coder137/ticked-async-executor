use ticked_async_executor::TickedAsyncExecutor;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_tracy::client::ProfiledAllocator;

#[global_allocator]
static GLOBAL: ProfiledAllocator<std::alloc::System> =
    ProfiledAllocator::new(std::alloc::System, 100);

async fn async_yield_now() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

fn main() {
    let tracy_layer = tracing_tracy::TracyLayer::default();
    tracing_subscriber::registry().with(tracy_layer).init();

    let mut executor = TickedAsyncExecutor::default();

    for i in 0..1 {
        executor
            .spawn_local(
                "MyIdentifier",
                async move {
                    let mut counter = 0;
                    loop {
                        println!("COUNT: {}", counter);
                        counter += 1;
                        async_yield_now().await;
                    }
                }
                .instrument(tracing::info_span!("Spawn", i)),
            )
            .detach();
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(16));
        executor.tick(16.00, None);
    }
}
