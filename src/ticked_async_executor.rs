use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use async_task::{Runnable, Task};

use crate::{DroppableFuture, TaskIdentifier};

#[derive(Debug)]
pub enum TaskState {
    Spawn(TaskIdentifier),
    Wake(TaskIdentifier),
    Tick(TaskIdentifier),
    Drop(TaskIdentifier),
}

type Payload = (TaskIdentifier, Runnable);

pub struct TickedAsyncExecutor<O> {
    channel: (mpsc::Sender<Payload>, mpsc::Receiver<Payload>),
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,
    observer: O,
}

impl<O> TickedAsyncExecutor<O>
where
    O: Fn(TaskState) + Clone + Send + Sync + 'static,
{
    pub fn new(observer: O) -> Self {
        Self {
            channel: mpsc::channel(),
            num_woken_tasks: Arc::new(AtomicUsize::new(0)),
            num_spawned_tasks: Arc::new(AtomicUsize::new(0)),
            observer,
        }
    }

    pub fn spawn<T>(
        &self,
        identifier: impl Into<TaskIdentifier>,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T>
    where
        T: Send + 'static,
    {
        let identifier = identifier.into();
        let future = self.droppable_future(identifier.clone(), future);
        let schedule = self.runnable_schedule_cb(identifier);
        let (runnable, task) = async_task::spawn(future, schedule);
        runnable.schedule();
        task
    }

    pub fn spawn_local<T>(
        &self,
        identifier: impl Into<TaskIdentifier>,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T>
    where
        T: 'static,
    {
        let identifier = identifier.into();
        let future = self.droppable_future(identifier.clone(), future);
        let schedule = self.runnable_schedule_cb(identifier);
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
            .for_each(|(identifier, runnable)| {
                (self.observer)(TaskState::Tick(identifier));
                runnable.run();
            });
        self.num_woken_tasks
            .fetch_sub(num_woken_tasks, Ordering::Relaxed);
    }

    fn droppable_future<F>(
        &self,
        identifier: TaskIdentifier,
        future: F,
    ) -> DroppableFuture<F, impl Fn()>
    where
        F: Future,
    {
        let observer = self.observer.clone();

        // Spawn Task
        self.num_spawned_tasks.fetch_add(1, Ordering::Relaxed);
        observer(TaskState::Spawn(identifier.clone()));

        // Droppable Future registering on_drop callback
        let num_spawned_tasks = self.num_spawned_tasks.clone();
        DroppableFuture::new(future, move || {
            num_spawned_tasks.fetch_sub(1, Ordering::Relaxed);
            observer(TaskState::Drop(identifier.clone()));
        })
    }

    fn runnable_schedule_cb(&self, identifier: TaskIdentifier) -> impl Fn(Runnable) {
        let sender = self.channel.0.clone();
        let num_woken_tasks = self.num_woken_tasks.clone();
        let observer = self.observer.clone();
        move |runnable| {
            sender.send((identifier.clone(), runnable)).unwrap_or(());
            num_woken_tasks.fetch_add(1, Ordering::Relaxed);
            observer(TaskState::Wake(identifier.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::join;

    use super::*;

    #[test]
    fn test_multiple_tasks() {
        let executor = TickedAsyncExecutor::new(|_state| {});
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

        executor.tick();
        assert_eq!(executor.num_tasks(), 2);

        executor.tick();
        assert_eq!(executor.num_tasks(), 0);
    }

    #[test]
    fn test_task_cancellation() {
        let executor = TickedAsyncExecutor::new(|_state| println!("{_state:?}"));
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
        executor.tick();

        executor
            .spawn_local("CancelTasks", async move {
                let (t1, t2) = join!(task1.cancel(), task2.cancel());
                assert_eq!(t1, None);
                assert_eq!(t2, None);
            })
            .detach();
        assert_eq!(executor.num_tasks(), 3);

        // Since we have cancelled the tasks above, the loops should eventually end
        while executor.num_tasks() != 0 {
            executor.tick();
        }
    }
}
