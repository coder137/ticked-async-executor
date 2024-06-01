use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use async_task::{Runnable, Task};

use crate::DroppableFuture;

pub struct TickedAsyncExecutor {
    channel: (mpsc::Sender<Runnable>, mpsc::Receiver<Runnable>),
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,
}

impl Default for TickedAsyncExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// TODO, Observer: Task spawn/wake/drop events
// TODO, Task Identifier String
impl TickedAsyncExecutor {
    pub fn new() -> Self {
        Self {
            channel: mpsc::channel(),
            num_woken_tasks: Arc::new(AtomicUsize::new(0)),
            num_spawned_tasks: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn spawn<T>(&self, future: impl Future<Output = T> + Send + 'static) -> Task<T>
    where
        T: Send + 'static,
    {
        let future = self.droppable_future(future);
        let schedule = self.runnable_schedule_cb();
        let (runnable, task) = async_task::spawn(future, schedule);
        runnable.schedule();
        task
    }

    pub fn spawn_local<T>(&self, future: impl Future<Output = T> + 'static) -> Task<T>
    where
        T: 'static,
    {
        let future = self.droppable_future(future);
        let schedule = self.runnable_schedule_cb();
        let (runnable, task) = async_task::spawn_local(future, schedule);
        runnable.schedule();
        task
    }

    pub fn num_tasks(&self) -> usize {
        self.num_spawned_tasks.load(Ordering::Relaxed)
    }

    /// Run the woken tasks once
    ///
    /// NOTE: Will not run tasks that are woken/scheduled immediately after `Runnable::run`
    pub fn tick(&self) {
        let num_woken_tasks = self.num_woken_tasks.load(Ordering::Relaxed);
        self.channel
            .1
            .try_iter()
            .take(num_woken_tasks)
            .for_each(|runnable| {
                runnable.run();
            });
        self.num_woken_tasks
            .fetch_sub(num_woken_tasks, Ordering::Relaxed);
    }

    fn droppable_future<F>(&self, future: F) -> DroppableFuture<F, impl Fn()>
    where
        F: Future,
    {
        self.num_spawned_tasks.fetch_add(1, Ordering::Relaxed);
        let num_spawned_tasks = self.num_spawned_tasks.clone();
        DroppableFuture::new(future, move || {
            num_spawned_tasks.fetch_sub(1, Ordering::Relaxed);
        })
    }

    fn runnable_schedule_cb(&self) -> impl Fn(Runnable) {
        let sender = self.channel.0.clone();
        let num_woken_tasks = self.num_woken_tasks.clone();
        move |runnable| {
            sender.send(runnable).unwrap_or(());
            num_woken_tasks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple_tasks() {
        let executor = TickedAsyncExecutor::new();
        executor
            .spawn_local(async move {
                println!("A: Start");
                tokio::task::yield_now().await;
                println!("A: End");
            })
            .detach();

        executor
            .spawn_local(async move {
                println!("B: Start");
                tokio::task::yield_now().await;
                println!("B: End");
            })
            .detach();

        // A, B, C: Start
        executor.tick();
        assert_eq!(executor.num_tasks(), 2);

        // A, B, C: End
        executor.tick();
        assert_eq!(executor.num_tasks(), 0);
    }

    #[test]
    fn test_task_cancellation() {
        let executor = TickedAsyncExecutor::new();
        let task1 = executor.spawn_local(async move {
            loop {
                println!("A: Start");
                tokio::task::yield_now().await;
                println!("A: End");
            }
        });

        let task2 = executor.spawn_local(async move {
            loop {
                println!("B: Start");
                tokio::task::yield_now().await;
                println!("B: End");
            }
        });
        assert_eq!(executor.num_tasks(), 2);
        executor.tick();

        executor
            .spawn_local(async move {
                task1.cancel().await;
                task2.cancel().await;
            })
            .detach();
        assert_eq!(executor.num_tasks(), 3);

        // Since we have cancelled the tasks above, the loops should eventually end
        while executor.num_tasks() != 0 {
            executor.tick();
        }
    }
}
