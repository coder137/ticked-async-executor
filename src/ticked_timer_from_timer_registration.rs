use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Default)]
pub struct TimerSystem {
    events: RefCell<VecDeque<Rc<futures_intrusive::sync::LocalManualResetEvent>>>,
    timers: RefCell<
        slotmap::SlotMap<
            slotmap::DefaultKey,
            (f64, Rc<futures_intrusive::sync::LocalManualResetEvent>),
        >,
    >,
}

impl TimerSystem {
    pub fn resolve_timers(&self, delta: f64) {
        self.timers.borrow_mut().retain(|_, (elapsed, event)| {
            *elapsed -= delta;
            if *elapsed <= 0.0 {
                event.set();
            }
            *elapsed > 0.0
        });
    }

    fn add_timer(
        &self,
        duration: f64,
        event: Rc<futures_intrusive::sync::LocalManualResetEvent>,
    ) -> slotmap::DefaultKey {
        self.timers.borrow_mut().insert((duration, event))
    }

    fn remove_timer(&self, key: slotmap::DefaultKey) {
        self.timers.borrow_mut().remove(key);
    }

    fn take_event(&self) -> Rc<futures_intrusive::sync::LocalManualResetEvent> {
        match self.events.borrow_mut().pop_front() {
            Some(event) => {
                event.reset();
                event
            }
            None => Rc::new(futures_intrusive::sync::LocalManualResetEvent::new(false)),
        }
    }

    fn give_event(&self, event: Rc<futures_intrusive::sync::LocalManualResetEvent>) {
        self.events.borrow_mut().push_back(event);
    }
}

pub struct TickedTimerFromTimerRegistration {
    timer_system: Rc<TimerSystem>,
}

impl TickedTimerFromTimerRegistration {
    pub fn new(timer_system: Rc<TimerSystem>) -> Self {
        Self { timer_system }
    }

    pub async fn sleep_for(&self, duration: f64) {
        let event = self.timer_system.take_event();
        let timer_key = self.timer_system.add_timer(duration, event.clone());

        let timer_system = self.timer_system.clone();
        crate::DroppableFuture::new(event.clone().wait(), || {
            timer_system.remove_timer(timer_key);
            timer_system.give_event(event);
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use crate::TickedAsyncExecutor;

    #[test]
    fn test_timer_registration_success() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.events.borrow().len(), 0);
        assert_eq!(timer.timer_system.timers.borrow().len(), 0);

        executor
            .spawn_local((), async move {
                println!("TASK BEFORE");
                timer.sleep_for(10.0).await;
                println!("TASK AFTER");
            })
            .detach();

        for i in 0..10 {
            println!("TICK BEFORE: {i}");
            assert_eq!(executor.num_tasks(), 1);
            executor.tick_all(1.0);
            println!("TICK AFTER: {i}");
            assert_eq!(executor.num_tasks(), 1);
        }
        println!("TICK BEFORE: 10");
        assert_eq!(executor.num_tasks(), 1);
        executor.tick_all(1.0);
        println!("TICK AFTER: 10");
        assert_eq!(executor.num_tasks(), 0);

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.events.borrow().len(), 1);
        assert_eq!(timer.timer_system.timers.borrow().len(), 0);
    }

    #[test]
    fn test_timer_registration_cancellation_safety() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.events.borrow().len(), 0);
        assert_eq!(timer.timer_system.timers.borrow().len(), 0);

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        executor
            .spawn_local("_", async move {
                //
                let timer_future = timer.sleep_for(10.0);
                let out = tokio::select! {
                    _ = timer_future => {
                        println!("TIMER completed");
                        1
                    }
                    _ = rx => {
                        println!("RX completed");
                        2
                    }
                };
                assert_eq!(out, 2);
            })
            .detach();

        executor.tick_all(1.0);
        assert_eq!(executor.num_tasks(), 1);

        executor.tick_all(1.0);
        assert_eq!(executor.num_tasks(), 1);

        tx.send(()).unwrap();
        executor.tick_all(1.0);
        assert_eq!(executor.num_tasks(), 0);
        println!("DONE");

        // NOTE, check if we have 1 event in the resource pool
        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.events.borrow().len(), 1);
        assert_eq!(timer.timer_system.timers.borrow().len(), 0);
    }
}
