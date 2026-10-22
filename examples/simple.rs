use std::time::Instant;

use ticked_async_executor::{TaskId, TaskObserver, TickedAsyncExecutor};

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
    struct DefaultObserver;
    impl TaskObserver for DefaultObserver {
        fn on_spawn(&self, id: &TaskId) {
            tracing::info!("on_spawn: {:?}", id);
        }

        fn on_wake(&self, id: &TaskId) {
            tracing::info!("on_wake: {:?}", id);
        }

        fn on_tick_start(&self, id: &TaskId, delta: f64) {
            tracing::info!("on_tick_start: {:?} {}", id, delta);
        }

        fn on_tick_end(&self, id: &TaskId) {
            tracing::info!("on_tick_end: {:?}", id);
        }

        fn on_drop(&self, id: &TaskId) {
            tracing::info!("on_drop: {:?}", id);
        }
    }
    let mut executor = TickedAsyncExecutor::new(DefaultObserver);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<usize>();
    executor
        .spawn_local("A", async move {
            loop {
                let d = rx.recv().await;
                println!("D: {:?}", d);
                if d.is_none() {
                    break;
                }
            }
        })
        .detach();
    executor
        .spawn_local("B", async move {
            for _i in 0..2 {
                let _ignore = tx.send(10);
                async_yield_now().await;
            }
        })
        .detach();

    for _i in 0..10 {
        println!("TICK START");
        executor.tick_all(1.0);
        println!("TICK END");
    }
}
