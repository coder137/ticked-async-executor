use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use crate::{DroppableFuture, TaskIdentifier, TickedTimer};

#[derive(Debug)]
pub enum TaskState {
    Spawn(TaskIdentifier),
    Wake(TaskIdentifier),
    Tick(TaskIdentifier, f64),
    Drop(TaskIdentifier),
}

pub type Task<T> = async_task::Task<T>;
type Payload = (TaskIdentifier, async_task::Runnable);

pub struct TickedAsyncExecutor<O> {
    channel: (mpsc::Sender<Payload>, mpsc::Receiver<Payload>),
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,

    // TODO, Or we need a Single Producer - Multi Consumer channel i.e Broadcast channel
    // Broadcast recv channel should be notified when there are new messages in the queue
    // Broadcast channel must also be able to remove older/stale messages (like a RingBuffer)
    observer: O,

    tick_event: tokio::sync::watch::Sender<f64>,
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
            tick_event: tokio::sync::watch::channel(1.0).0,
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
    /// `delta` is used for timing based operations
    /// - `TickedTimer` uses this delta value to tick till completion
    ///
    /// `maybe_limit` is used to limit the number of woken tasks run per tick
    /// - None would imply that there is no limit (all woken tasks would run)
    /// - Some(limit) would imply that [0..limit] woken tasks would run,
    /// even if more tasks are woken.
    ///
    /// Tick is !Sync i.e cannot be invoked from multiple threads
    ///
    /// NOTE: Will not run tasks that are woken/scheduled immediately after `Runnable::run`
    pub fn tick(&self, delta_in_ms: f64, maybe_limit: Option<usize>) {
        let _r = self.tick_event.send(delta_in_ms);

        // Clamp woken tasks to limit
        let mut num_woken_tasks = self.num_woken_tasks.load(Ordering::Relaxed);
        if let Some(limit) = maybe_limit {
            num_woken_tasks = num_woken_tasks.clamp(0, limit);
        }
        self.channel
            .1
            .try_iter()
            .take(num_woken_tasks)
            .for_each(|(identifier, runnable)| {
                (self.observer)(TaskState::Tick(identifier, delta_in_ms));
                runnable.run();
            });
        self.num_woken_tasks
            .fetch_sub(num_woken_tasks, Ordering::Relaxed);
    }

    pub fn create_timer(&self) -> TickedTimer {
        let tick_recv = self.tick_event.subscribe();
        TickedTimer { tick_recv }
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

    fn runnable_schedule_cb(&self, identifier: TaskIdentifier) -> impl Fn(async_task::Runnable) {
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
    use super::*;
    use std::time::{Duration, Instant};

    const DELTA: f64 = 1000.0 / 60.0;

    #[test]
    fn test_multiple_tasks() {
        let executor = TickedAsyncExecutor::default();
        executor
            .spawn("A", async move {
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
        while executor.num_tasks() != 0 {
            executor.tick(DELTA, None);
        }
    }

    #[test]
    fn test_ticked_timer() {
        let executor = TickedAsyncExecutor::default();

        for _ in 0..10 {
            let timer: TickedTimer = executor.create_timer();
            executor
                .spawn("ThreadedTimer", async move {
                    timer.sleep_for(256.0).await;
                })
                .detach();
        }

        for _ in 0..10 {
            let timer = executor.create_timer();
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
    }
}
