#[derive(Clone)]
pub struct TickedTimerFromTimerRegistration {
    timer_registration_tx: flume::Sender<(f64, std::task::Waker)>,
}

impl TickedTimerFromTimerRegistration {
    pub fn new(timer_registration_tx: flume::Sender<(f64, std::task::Waker)>) -> Self {
        Self {
            timer_registration_tx,
        }
    }

    pub fn sleep_for(&self, duration: f64) -> SleepFuture {
        SleepFuture {
            tx: self.timer_registration_tx.clone(),
            duration,
            send: true,
        }
    }
}

pub struct SleepFuture {
    tx: flume::Sender<(f64, std::task::Waker)>,
    duration: f64,

    // state
    send: bool,
}

impl std::future::Future for SleepFuture {
    type Output = Result<(), ()>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.send {
            self.send = false;
            let waker = cx.waker().clone();
            let _r = self.tx.send((self.duration, waker));
            if _r.is_err() {
                std::task::Poll::Ready(Err(()))
            } else {
                std::task::Poll::Pending
            }
        } else {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{SplitTickedAsyncExecutor, TickedAsyncExecutor};

    #[test]
    fn test_timer_registration_success() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
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
    fn test_timer_registration_failure() {
        let (spawner, ticker) = SplitTickedAsyncExecutor::default();

        let timer = spawner.create_timer_from_timer_registration();

        let mut timer_future = timer.sleep_for(10.0);
        drop(ticker);

        let waker = std::task::Waker::noop();
        let mut ctx = std::task::Context::from_waker(waker);
        let status = std::pin::pin!(&mut timer_future).poll(&mut ctx);
        assert_eq!(status, std::task::Poll::Ready(Err(())));
    }
}
