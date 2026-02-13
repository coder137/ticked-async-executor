use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

use crate::{DroppableFuture, TaskIdentifier};

#[derive(Debug)]
pub enum TaskState {
    Spawn(TaskIdentifier),
    Wake(TaskIdentifier),
    Tick(TaskIdentifier, f64),
    Drop(TaskIdentifier),
}

pub type Task<T> = async_task::Task<T>;
type Payload = (TaskIdentifier, async_task::Runnable);

pub struct SplitTickedAsyncExecutor;

impl SplitTickedAsyncExecutor {
    pub fn default() -> (
        TickedAsyncExecutorSpawner<fn(TaskState)>,
        TickedAsyncExecutorTicker<fn(TaskState)>,
    ) {
        Self::new(|_state| {})
    }

    pub fn new<O>(observer: O) -> (TickedAsyncExecutorSpawner<O>, TickedAsyncExecutorTicker<O>)
    where
        O: Fn(TaskState) + Clone + Send + Sync + 'static,
    {
        let (task_tx, task_rx) = mpsc::channel();
        let num_woken_tasks = Arc::new(AtomicUsize::new(0));
        let num_spawned_tasks = Arc::new(AtomicUsize::new(0));

        #[cfg(feature = "tick_event")]
        let (tick_event_tx, tick_event_rx) = tokio::sync::watch::channel(1.0);

        #[cfg(feature = "timer_registration")]
        let (timer_registration_tx, timer_registration_rx) = mpsc::channel();

        let spawner = TickedAsyncExecutorSpawner {
            task_tx,
            num_woken_tasks: num_woken_tasks.clone(),
            num_spawned_tasks: num_spawned_tasks.clone(),
            observer: observer.clone(),
            #[cfg(feature = "tick_event")]
            tick_event_rx,
            #[cfg(feature = "timer_registration")]
            timer_registration_tx,
        };
        let ticker = TickedAsyncExecutorTicker {
            task_rx,
            num_woken_tasks,
            num_spawned_tasks,
            observer,
            delta: Rc::new(0.0.into()),
            #[cfg(feature = "tick_event")]
            tick_event_tx,
            #[cfg(feature = "timer_registration")]
            timer_registration_rx,
            #[cfg(feature = "timer_registration")]
            timers: Vec::new(),
        };
        (spawner, ticker)
    }
}

pub struct TickedAsyncExecutorSpawner<O> {
    task_tx: mpsc::Sender<Payload>,
    num_woken_tasks: Arc<AtomicUsize>,

    num_spawned_tasks: Arc<AtomicUsize>,
    // TODO, Or we need a Single Producer - Multi Consumer channel i.e Broadcast channel
    // Broadcast recv channel should be notified when there are new messages in the queue
    // Broadcast channel must also be able to remove older/stale messages (like a RingBuffer)
    observer: O,

    #[cfg(feature = "tick_event")]
    tick_event_rx: tokio::sync::watch::Receiver<f64>,
    #[cfg(feature = "timer_registration")]
    timer_registration_tx: mpsc::Sender<(f64, tokio::sync::oneshot::Sender<()>)>,
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

    #[cfg(feature = "tick_event")]
    pub fn create_timer_from_tick_event(&self) -> crate::TickedTimerFromTickEvent {
        crate::TickedTimerFromTickEvent::new(self.tick_event_rx.clone())
    }

    #[cfg(feature = "tick_event")]
    pub fn tick_channel(&self) -> tokio::sync::watch::Receiver<f64> {
        self.tick_event_rx.clone()
    }

    #[cfg(feature = "timer_registration")]
    pub fn create_timer_from_timer_registration(&self) -> crate::TickedTimerFromTimerRegistration {
        crate::TickedTimerFromTimerRegistration::new(self.timer_registration_tx.clone())
    }

    pub fn num_tasks(&self) -> usize {
        self.num_spawned_tasks.load(Ordering::Relaxed)
    }

    fn droppable_future<F>(
        &self,
        identifier: TaskIdentifier,
        future: F,
    ) -> DroppableFuture<F, impl Fn() + use<F, O>>
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

    fn runnable_schedule_cb(
        &self,
        identifier: TaskIdentifier,
    ) -> impl Fn(async_task::Runnable) + use<O> {
        let sender = self.task_tx.clone();
        let num_woken_tasks = self.num_woken_tasks.clone();
        let observer = self.observer.clone();
        move |runnable| {
            sender.send((identifier.clone(), runnable)).unwrap_or(());
            num_woken_tasks.fetch_add(1, Ordering::Relaxed);
            observer(TaskState::Wake(identifier.clone()));
        }
    }
}

#[derive(Clone)]
pub struct TickedAsyncExecutorDelta(Rc<Cell<f64>>);

impl TickedAsyncExecutorDelta {
    pub fn get(&self) -> f64 {
        self.0.get()
    }

    pub fn inner(self) -> Rc<Cell<f64>> {
        self.0
    }
}

pub struct TickedAsyncExecutorTicker<O> {
    task_rx: mpsc::Receiver<Payload>,
    num_woken_tasks: Arc<AtomicUsize>,
    num_spawned_tasks: Arc<AtomicUsize>,
    observer: O,
    delta: Rc<Cell<f64>>,

    #[cfg(feature = "tick_event")]
    tick_event_tx: tokio::sync::watch::Sender<f64>,

    #[cfg(feature = "timer_registration")]
    timer_registration_rx: mpsc::Receiver<(f64, tokio::sync::oneshot::Sender<()>)>,
    #[cfg(feature = "timer_registration")]
    timers: Vec<(f64, tokio::sync::oneshot::Sender<()>)>,
}

impl<O> TickedAsyncExecutorTicker<O>
where
    O: Fn(TaskState),
{
    pub fn delta(&self) -> TickedAsyncExecutorDelta {
        TickedAsyncExecutorDelta(self.delta.clone())
    }

    pub fn tick(&mut self, delta: f64, limit: Option<usize>) {
        self.delta.replace(delta);

        #[cfg(feature = "tick_event")]
        let _r = self.tick_event_tx.send(delta);

        #[cfg(feature = "timer_registration")]
        self.timer_registration_tick(delta);

        let mut num_woken_tasks = self.num_woken_tasks.load(Ordering::Relaxed);
        if let Some(limit) = limit {
            // Woken tasks should not exceed the allowed limit
            num_woken_tasks = num_woken_tasks.min(limit);
        }

        self.task_rx
            .try_iter()
            .take(num_woken_tasks)
            .for_each(|(identifier, runnable)| {
                (self.observer)(TaskState::Tick(identifier, delta));
                runnable.run();
            });
        self.num_woken_tasks
            .fetch_sub(num_woken_tasks, Ordering::Relaxed);
    }

    pub fn wait_till_completed(&mut self, constant_delta: f64) {
        while self.num_spawned_tasks.load(Ordering::Relaxed) != 0 {
            self.tick(constant_delta, None);
        }
    }

    #[cfg(feature = "timer_registration")]
    fn timer_registration_tick(&mut self, delta: f64) {
        // Get new timers
        self.timer_registration_rx.try_iter().for_each(|timer| {
            self.timers.push(timer);
        });

        // Countdown timers
        if self.timers.is_empty() {
            return;
        }
        self.timers.iter_mut().for_each(|(elapsed, _)| {
            *elapsed -= delta;
        });

        // Extract timers that have elapsed
        // Notify corresponding channels
        self.timers
            .extract_if(.., |(elapsed, _)| *elapsed <= 0.0)
            .for_each(|(_, rx)| {
                let _ignore = rx.send(());
            });
    }
}
