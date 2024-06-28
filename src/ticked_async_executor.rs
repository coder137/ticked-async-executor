use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use crate::TaskIdentifier;

#[derive(Debug)]
pub enum TaskState {
    Spawn(TaskIdentifier),
    Wake(TaskIdentifier),
    Tick(TaskIdentifier),
    Drop(TaskIdentifier),
}

pub type Task<T, O> = async_task::Task<T, TaskMetadata<O>>;
type TaskRunnable<O> = async_task::Runnable<TaskMetadata<O>>;
type Payload<O> = (TaskIdentifier, TaskRunnable<O>);

/// Task Metadata associated with TickedAsyncExecutor
///
/// Primarily used to track when the Task is completed/cancelled
pub struct TaskMetadata<O>
where
    O: Fn(TaskState) + Send + Sync + 'static,
{
    num_spawned_tasks: Arc<AtomicUsize>,
    identifier: TaskIdentifier,
    observer: O,
}

impl<O> Drop for TaskMetadata<O>
where
    O: Fn(TaskState) + Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.num_spawned_tasks.fetch_sub(1, Ordering::Relaxed);
        (self.observer)(TaskState::Drop(self.identifier.clone()));
    }
}

pub struct TickedAsyncExecutor<O>
where
    O: Fn(TaskState) + Send + Sync + 'static,
{
    channel: (mpsc::Sender<Payload<O>>, mpsc::Receiver<Payload<O>>),
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,

    // TODO, Or we need a Single Producer - Multi Consumer channel i.e Broadcast channel
    // Broadcast recv channel should be notified when there are new messages in the queue
    // Broadcast channel must also be able to remove older/stale messages (like a RingBuffer)
    observer: O,
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
    ) -> Task<T, O>
    where
        T: Send + 'static,
    {
        let identifier = identifier.into();
        self.num_spawned_tasks.fetch_add(1, Ordering::Relaxed);
        (self.observer)(TaskState::Spawn(identifier.clone()));

        let schedule = self.runnable_schedule_cb(identifier.clone());
        let (runnable, task) = async_task::Builder::new()
            .metadata(TaskMetadata {
                num_spawned_tasks: self.num_spawned_tasks.clone(),
                identifier,
                observer: self.observer.clone(),
            })
            .spawn(|_m| future, schedule);
        runnable.schedule();
        task
    }

    pub fn spawn_local<T>(
        &self,
        identifier: impl Into<TaskIdentifier>,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T, O>
    where
        T: 'static,
    {
        let identifier = identifier.into();
        self.num_spawned_tasks.fetch_add(1, Ordering::Relaxed);
        (self.observer)(TaskState::Spawn(identifier.clone()));

        let schedule = self.runnable_schedule_cb(identifier.clone());
        let (runnable, task) = async_task::Builder::new()
            .metadata(TaskMetadata {
                num_spawned_tasks: self.num_spawned_tasks.clone(),
                identifier,
                observer: self.observer.clone(),
            })
            .spawn_local(move |_m| future, schedule);
        runnable.schedule();
        task
    }

    pub fn num_tasks(&self) -> usize {
        self.num_spawned_tasks.load(Ordering::Relaxed)
    }

    /// Run the woken tasks once
    ///
    /// Tick is !Sync i.e cannot be invoked from multiple threads
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

    fn runnable_schedule_cb(&self, identifier: TaskIdentifier) -> impl Fn(TaskRunnable<O>) {
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
        let executor = TickedAsyncExecutor::default();
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
