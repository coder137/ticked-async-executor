/// DO NOT USE
///
/// Use `TickedTimerFromTimerRegistration` gated under the `timer_registration` feature
pub struct TickedTimerFromTickEvent {
    tick_event_rx: tokio::sync::watch::Receiver<f64>,
}

impl TickedTimerFromTickEvent {
    pub fn new(tick_event_rx: tokio::sync::watch::Receiver<f64>) -> Self {
        Self { tick_event_rx }
    }

    pub async fn sleep_for(mut self, mut duration_in_ms: f64) -> Result<(), ()> {
        loop {
            let _r = self.tick_event_rx.changed().await;
            if _r.is_err() {
                // This means that the executor supplying the delta channel has shutdown
                // We must stop waiting gracefully
                break Err(());
            }
            let current_dt = *self.tick_event_rx.borrow_and_update();
            duration_in_ms -= current_dt;
            if duration_in_ms <= 0.0 {
                break Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{SplitTickedAsyncExecutor, TickedAsyncExecutor};

    #[test]
    fn test_timer_tick_success() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_tick_event();
        executor
            .spawn_local((), async move {
                println!("TASK BEFORE");
                timer.sleep_for(10.0).await.unwrap_or_default();
                println!("TASK AFTER");
            })
            .detach();

        for i in 0..=10 {
            println!("TICK BEFORE: {i}");
            executor.tick(1.0, None);
            println!("TICK AFTER: {i}");
        }
        assert_eq!(executor.num_tasks(), 0);
    }

    #[test]
    fn test_timer_tick_failure() {
        let (spawner, ticker) = SplitTickedAsyncExecutor::default();

        let timer = spawner.create_timer_from_tick_event();
        drop(ticker);

        let timer_future = timer.sleep_for(10.0);
        let mut timer_future = std::pin::pin!(timer_future);

        let waker = std::task::Waker::noop();
        let mut ctx = std::task::Context::from_waker(waker);
        let status = timer_future.as_mut().poll(&mut ctx);
        assert_eq!(status, std::task::Poll::Ready(Err(())));
    }
}
