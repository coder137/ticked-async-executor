use std::future::Future;

use crate::{
    SplitTickedAsyncExecutor, Task, TaskIdentifier, TaskState, TickedAsyncExecutorSpawner,
    TickedAsyncExecutorTicker,
};

pub struct TickedAsyncExecutor<O> {
    spawner: TickedAsyncExecutorSpawner<O>,
    ticker: TickedAsyncExecutorTicker<O>,
}

impl Default for TickedAsyncExecutor<fn(TaskState)> {
    fn default() -> Self {
        Self::new(|_| {})
    }
}

impl<O> TickedAsyncExecutor<O>
where
    O: Fn(TaskState) + Clone + Send + Sync + 'static,
{
    pub fn new(observer: O) -> Self {
        let (spawner, ticker) = SplitTickedAsyncExecutor::new(observer);
        Self { spawner, ticker }
    }

    pub fn spawn_local<T>(
        &self,
        identifier: impl Into<TaskIdentifier>,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T>
    where
        T: 'static,
    {
        self.spawner.spawn_local(identifier, future)
    }

    pub fn num_tasks(&self) -> usize {
        self.spawner.num_tasks()
    }

    /// Run the woken tasks once
    ///
    /// `delta` is used for timing based operations
    /// - `TickedTimer` uses this delta value to tick till completion
    ///
    /// `limit` is used to limit the number of woken tasks run per tick
    /// - None would imply that there is no limit (all woken tasks would run)
    /// - Some(limit) would imply that [0..limit] woken tasks would run,
    ///   even if more tasks are woken.
    ///
    /// Tick is !Sync i.e cannot be invoked from multiple threads
    ///
    /// NOTE: Will not run tasks that are woken/scheduled immediately after `Runnable::run`
    pub fn tick(&mut self, delta: f64, limit: Option<usize>) {
        self.ticker.tick(delta, limit);
    }

    #[cfg(feature = "tick_event")]
    pub fn create_timer_from_tick_event(&self) -> crate::TickedTimerFromTickEvent {
        self.spawner.create_timer_from_tick_event()
    }

    #[cfg(feature = "tick_event")]
    pub fn tick_channel(&self) -> tokio::sync::watch::Receiver<f64> {
        self.spawner.tick_channel()
    }

    pub fn wait_till_completed(&mut self, delta: f64) {
        self.ticker.wait_till_completed(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELTA: f64 = 1000.0 / 60.0;

    #[test]
    fn test_one_task() {
        const DELTA: f64 = 1.0 / 60.0;
        const LIMIT: Option<usize> = None;

        let mut executor = TickedAsyncExecutor::default();

        executor.spawn_local("MyIdentifier", async move {}).detach();

        // Make sure to tick your executor to run the tasks
        executor.tick(DELTA, LIMIT);
        assert_eq!(executor.num_tasks(), 0);
    }

    #[test]
    fn test_multiple_tasks() {
        let mut executor = TickedAsyncExecutor::default();
        executor
            .spawn_local("A", async move {
                tokio::task::yield_now().await;
            })
            .detach();

        executor
            .spawn_local(format!("B"), async move {
                tokio::task::yield_now().await;
            })
            .detach();

        executor.tick(DELTA, None);
        assert_eq!(executor.num_tasks(), 2);

        executor.tick(DELTA, None);
        assert_eq!(executor.num_tasks(), 0);
    }

    #[test]
    fn test_task_cancellation() {
        let mut executor = TickedAsyncExecutor::new(|_state| println!("{_state:?}"));
        let task1 = executor.spawn_local("A", async move {
            loop {
                tokio::task::yield_now().await;
            }
        });

        let task2 = executor.spawn_local(format!("B"), async move {
            loop {
                tokio::task::yield_now().await;
            }
        });
        assert_eq!(executor.num_tasks(), 2);
        executor.tick(DELTA, None);

        executor
            .spawn_local("CancelTasks", async move {
                let (t1, t2) = tokio::join!(task1.cancel(), task2.cancel());
                assert_eq!(t1, None);
                assert_eq!(t2, None);
            })
            .detach();
        assert_eq!(executor.num_tasks(), 3);

        // Since we have cancelled the tasks above, the loops should eventually end
        executor.wait_till_completed(DELTA);
    }

    #[cfg(feature = "tick_event")]
    #[test]
    fn test_ticked_timer() {
        use std::time::{Duration, Instant};

        let mut executor = TickedAsyncExecutor::default();

        for _ in 0..10 {
            let timer = executor.create_timer_from_tick_event();
            executor
                .spawn_local("LocalTimer", async move {
                    timer.sleep_for(256.0).await;
                })
                .detach();
        }

        let now = Instant::now();
        let mut instances = vec![];
        while executor.num_tasks() != 0 {
            let current = Instant::now();
            executor.tick(DELTA, None);
            instances.push(current.elapsed());
            std::thread::sleep(Duration::from_millis(16));
        }
        let elapsed = now.elapsed();
        println!("Elapsed: {:?}", elapsed);
        println!("Total: {:?}", instances);
        println!(
            "Min: {:?}, Max: {:?}",
            instances.iter().min(),
            instances.iter().max()
        );

        // Test Timer cancellation
        let timer = executor.create_timer_from_tick_event();
        executor
            .spawn_local("LocalFuture1", async move {
                timer.sleep_for(1000.0).await;
            })
            .detach();

        let timer = executor.create_timer_from_tick_event();
        executor
            .spawn_local("LocalFuture2", async move {
                timer.sleep_for(1000.0).await;
            })
            .detach();

        let mut tick_event = executor.tick_channel();
        executor
            .spawn_local("LocalTickFuture1", async move {
                loop {
                    let _r = tick_event.changed().await;
                    if _r.is_err() {
                        break;
                    }
                }
            })
            .detach();

        let mut tick_event = executor.tick_channel();
        executor
            .spawn_local("LocalTickFuture2", async move {
                loop {
                    let _r = tick_event.changed().await;
                    if _r.is_err() {
                        break;
                    }
                }
            })
            .detach();

        executor.tick(DELTA, None);
        assert_eq!(executor.num_tasks(), 4);
        drop(executor);
    }

    #[test]
    fn test_limit() {
        let mut executor = TickedAsyncExecutor::default();
        for i in 0..10 {
            executor
                .spawn_local(format!("{i}"), async move {
                    println!("Finish {i}");
                })
                .detach();
        }

        for i in 0..10 {
            let num_tasks = executor.num_tasks();
            assert_eq!(num_tasks, 10 - i);
            executor.tick(0.1, Some(1));
        }

        assert_eq!(executor.num_tasks(), 0);
    }
}
