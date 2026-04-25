use ticked_async_executor::TickedAsyncExecutor;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_tracy::client::ProfiledAllocator;

#[global_allocator]
static GLOBAL: ProfiledAllocator<std::alloc::System> =
    ProfiledAllocator::new(std::alloc::System, 100);

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

    let timer = executor.create_timer_from_timer_registration();
    for i in 0..10 {
        let timer_clone = timer.clone();
        let shutdown_clone = shutdown.clone();
        executor
            .spawn_local(
                (),
                async move {
                    let mut counter = 0;
                    loop {
                        println!("COUNT: {}", counter);
                        counter += 1;
                        timer_clone.sleep_for(160.0).await.unwrap_or_default();
                        if shutdown_clone.load(std::sync::atomic::Ordering::Relaxed) {
                            println!("SHUTDOWN: {i}");
                            break;
                        }
                    }
                }
                .instrument(tracing::info_span!("Task", name = i)),
            )
            .detach();
    }

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
