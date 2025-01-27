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

pub fn new_split_ticked_async_executor<O>(
    observer: O,
) -> (TickedAsyncExecutorSpawner<O>, TickedAsyncExecutorTicker<O>)
where
    O: Fn(TaskState) + Clone + Send + Sync + 'static,
{
    let (tx_channel, rx_channel) = mpsc::channel();
    let num_woken_tasks = Arc::new(AtomicUsize::new(0));
    let num_spawned_tasks = Arc::new(AtomicUsize::new(0));
    let (tx_tick_event, rx_tick_event) = tokio::sync::watch::channel(1.0);
    let spawner = TickedAsyncExecutorSpawner {
        tx_channel,
        num_woken_tasks: num_woken_tasks.clone(),
        num_spawned_tasks: num_spawned_tasks.clone(),
        observer: observer.clone(),
        rx_tick_event,
    };
    let ticker = TickedAsyncExecutorTicker {
        rx_channel,
        num_woken_tasks,
        num_spawned_tasks,
        observer,
        tx_tick_event,
    };
    (spawner, ticker)
}

pub struct TickedAsyncExecutorSpawner<O> {
    tx_channel: mpsc::Sender<Payload>,
    num_woken_tasks: Arc<AtomicUsize>,

    num_spawned_tasks: Arc<AtomicUsize>,
    // TODO, Or we need a Single Producer - Multi Consumer channel i.e Broadcast channel
    // Broadcast recv channel should be notified when there are new messages in the queue
    // Broadcast channel must also be able to remove older/stale messages (like a RingBuffer)
    observer: O,
    rx_tick_event: tokio::sync::watch::Receiver<f64>,
}

impl<O> TickedAsyncExecutorSpawner<O>
where
    O: Fn(TaskState) + Clone + Send + Sync + 'static,
{
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

    pub fn create_timer(&self) -> TickedTimer {
        let tick_recv = self.rx_tick_event.clone();
        TickedTimer { tick_recv }
    }

    pub fn tick_channel(&self) -> tokio::sync::watch::Receiver<f64> {
        self.rx_tick_event.clone()
    }

    pub fn num_tasks(&self) -> usize {
        self.num_spawned_tasks.load(Ordering::Relaxed)
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
        let sender = self.tx_channel.clone();
        let num_woken_tasks = self.num_woken_tasks.clone();
        let observer = self.observer.clone();
        move |runnable| {
            sender.send((identifier.clone(), runnable)).unwrap_or(());
            num_woken_tasks.fetch_add(1, Ordering::Relaxed);
            observer(TaskState::Wake(identifier.clone()));
        }
    }
}

pub struct TickedAsyncExecutorTicker<O> {
    rx_channel: mpsc::Receiver<Payload>,
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,
    observer: O,
    tx_tick_event: tokio::sync::watch::Sender<f64>,
}

impl<O> TickedAsyncExecutorTicker<O>
where
    O: Fn(TaskState),
{
    pub fn tick(&self, delta: f64, limit: Option<usize>) {
        let _r = self.tx_tick_event.send(delta);

        let mut num_woken_tasks = self.num_woken_tasks.load(Ordering::Relaxed);
        if let Some(limit) = limit {
            // Woken tasks should not exceed the allowed limit
            num_woken_tasks = num_woken_tasks.min(limit);
        }

        self.rx_channel
            .try_iter()
            .take(num_woken_tasks)
            .for_each(|(identifier, runnable)| {
                (self.observer)(TaskState::Tick(identifier, delta));
                runnable.run();
            });
        self.num_woken_tasks
            .fetch_sub(num_woken_tasks, Ordering::Relaxed);
    }

    pub fn wait_till_completed(&self, constant_delta: f64) {
        while self.num_spawned_tasks.load(Ordering::Relaxed) != 0 {
            self.tick(constant_delta, None);
        }
    }
}
