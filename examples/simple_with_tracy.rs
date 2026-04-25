use ticked_async_executor::TickedAsyncExecutor;
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

fn main() -> Result<(), ()> {
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    })
    .unwrap();

    let tracy_layer = tracing_tracy::TracyLayer::default();
    tracing_subscriber::registry().with(tracy_layer).init();

    let mut executor = TickedAsyncExecutor::new(|state| {
        tracing::info!("{state:?}");
    });

    executor
        .spawn_local((), async move {
            let mut counter = 0;
            loop {
                println!("COUNT: {}", counter);
                counter += 1;
                async_yield_now().await;
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("SHUTDOWN");
                    break;
                }
            }
        })
        .detach();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(16));
        executor.tick(16.00, None);
        if executor.num_tasks() == 0 {
            break;
        }
    }

    let profiler_connected = tracing_tracy::client::Client::is_connected();
    println!("Profiler Running: {profiler_connected}");
    if !profiler_connected {
        // Due to "flush-on-exit" we need to force exit
        std::process::exit(1);
    }
    Ok(())
}
