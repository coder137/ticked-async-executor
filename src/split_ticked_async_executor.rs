use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{DroppableFuture, TaskId, UserId};

#[derive(Debug)]
pub enum TaskState {
    Spawn(TaskId),
    Wake(TaskId),
    TickStart(TaskId, f64),
    TickEnd(TaskId),
    Drop(TaskId),
}

pub type Task<T> = async_task::Task<T>;
type Payload = (TaskId, async_task::Runnable);

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
        let (task_tx, task_rx) = flume::unbounded();
        let num_spawned_tasks = Arc::new(AtomicUsize::new(0));
        let delta = Rc::new(Cell::new(0.0));

        #[cfg(feature = "tick_event")]
        let (tick_event_tx, tick_event_rx) = tokio::sync::watch::channel(1.0);

        #[cfg(feature = "timer_registration")]
        let (timer_registration_tx, timer_registration_rx) = flume::unbounded();

        let spawner = TickedAsyncExecutorSpawner {
            task_tx,
            num_spawned_tasks: num_spawned_tasks.clone(),
            observer: observer.clone(),
            delta: delta.clone(),
            counter: Rc::new(Cell::new(0)),
            #[cfg(feature = "tick_event")]
            tick_event_rx,
            #[cfg(feature = "timer_registration")]
            timer_registration_tx,
            _not_send: std::marker::PhantomData,
        };
        let ticker = TickedAsyncExecutorTicker {
            task_rx,
            num_spawned_tasks,
            observer,
            delta,
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

pub struct TickedAsyncExecutorSpawner<O> {
    task_tx: flume::Sender<Payload>,
    num_spawned_tasks: Arc<AtomicUsize>,
    observer: O,
    delta: Rc<Cell<f64>>,
    counter: Rc<Cell<u64>>,

    #[cfg(feature = "tick_event")]
    tick_event_rx: tokio::sync::watch::Receiver<f64>,
    #[cfg(feature = "timer_registration")]
    timer_registration_tx: flume::Sender<(f64, std::task::Waker)>,

    // https://github.com/rust-lang/rust/issues/68318
    _not_send: std::marker::PhantomData<*const ()>,
}

impl<O: Clone> Clone for TickedAsyncExecutorSpawner<O> {
    fn clone(&self) -> Self {
        Self {
            task_tx: self.task_tx.clone(),
            num_spawned_tasks: self.num_spawned_tasks.clone(),
            observer: self.observer.clone(),
            delta: self.delta.clone(),
            counter: self.counter.clone(),
            #[cfg(feature = "tick_event")]
            tick_event_rx: self.tick_event_rx.clone(),
            #[cfg(feature = "timer_registration")]
            timer_registration_tx: self.timer_registration_tx.clone(),
            _not_send: self._not_send,
        }
    }
}

impl<O> TickedAsyncExecutorSpawner<O>
where
    O: Fn(TaskState) + Clone + Send + Sync + 'static,
{
    pub fn delta(&self) -> TickedAsyncExecutorDelta {
        TickedAsyncExecutorDelta(self.delta.clone())
    }

    pub fn spawn_local<T>(
        &self,
        user_id: impl Into<UserId>,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T>
    where
        T: 'static,
    {
        let next_id = self.counter.get() + 1;
        let task_id = self.counter.replace(next_id);
        let user_id = user_id.into();
        let task_id = TaskId { task_id, user_id };

        let future = self.droppable_future(task_id.clone(), future);
        let schedule = self.runnable_schedule_cb(task_id);
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
        task_id: TaskId,
        future: F,
    ) -> DroppableFuture<F, impl Fn() + use<F, O>>
    where
        F: Future,
    {
        let observer = self.observer.clone();

        // Spawn Task
        self.num_spawned_tasks.fetch_add(1, Ordering::Relaxed);
        observer(TaskState::Spawn(task_id.clone()));

        // Droppable Future registering on_drop callback
        let num_spawned_tasks = self.num_spawned_tasks.clone();
        DroppableFuture::new(future, move || {
            num_spawned_tasks.fetch_sub(1, Ordering::Relaxed);
            observer(TaskState::Drop(task_id.clone()));
        })
    }

    fn runnable_schedule_cb(&self, task_id: TaskId) -> impl Fn(async_task::Runnable) + use<O> {
        let task_tx = self.task_tx.clone();
        let observer = self.observer.clone();
        move |runnable| {
            task_tx.send((task_id.clone(), runnable)).unwrap_or(());
            observer(TaskState::Wake(task_id.clone()));
        }
    }
}

pub struct TickedAsyncExecutorTicker<O> {
    task_rx: flume::Receiver<Payload>,
    num_spawned_tasks: Arc<AtomicUsize>,
    observer: O,
    delta: Rc<Cell<f64>>,

    #[cfg(feature = "tick_event")]
    tick_event_tx: tokio::sync::watch::Sender<f64>,

    #[cfg(feature = "timer_registration")]
    timer_registration_rx: flume::Receiver<(f64, std::task::Waker)>,

    #[cfg(feature = "timer_registration")]
    timers: Vec<(f64, std::task::Waker)>,
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

        let mut num_woken_tasks = self.task_rx.len();
        if let Some(limit) = limit {
            // Woken tasks should not exceed the allowed limit
            num_woken_tasks = num_woken_tasks.min(limit);
        }

        self.task_rx
            .try_iter()
            .take(num_woken_tasks)
            .for_each(|(identifier, runnable)| {
                (self.observer)(TaskState::TickStart(identifier.clone(), delta));
                runnable.run();
                (self.observer)(TaskState::TickEnd(identifier))
            });
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

        // Update timers with delta
        // Extract timers that have elapsed
        // Notify corresponding channels
        self.timers
            .extract_if(.., |(elapsed, _)| {
                *elapsed -= delta;
                *elapsed <= 0.0
            })
            .for_each(|(_, waker)| {
                waker.wake();
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_ticked_async_executor_spawner_clone() {
        let (spawner, _ticker) = SplitTickedAsyncExecutor::default();

        let _spawner_clone = spawner.clone();
    }
}
