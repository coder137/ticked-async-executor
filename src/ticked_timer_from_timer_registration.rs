use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Default)]
pub struct TimerSystem {
    events: VecDeque<Rc<futures_intrusive::sync::LocalManualResetEvent>>,
    timers: slotmap::SlotMap<
        slotmap::DefaultKey,
        (f64, Rc<futures_intrusive::sync::LocalManualResetEvent>),
    >,
}

impl TimerSystem {
    pub fn resolve_timers(&mut self, delta: f64) {
        self.timers.retain(|_, (elapsed, event)| {
            *elapsed -= delta;
            if *elapsed <= 0.0 {
                event.set();
            }
            *elapsed > 0.0
        });
    }

    fn add_timer(
        &mut self,
        duration: f64,
        event: Rc<futures_intrusive::sync::LocalManualResetEvent>,
    ) -> slotmap::DefaultKey {
        self.timers.insert((duration, event))
    }

    fn remove_timer(&mut self, key: slotmap::DefaultKey) {
        self.timers.remove(key);
    }

    fn take_event(&mut self) -> Rc<futures_intrusive::sync::LocalManualResetEvent> {
        match self.events.pop_front() {
            Some(event) => {
                event.reset();
                event
            }
            None => Rc::new(futures_intrusive::sync::LocalManualResetEvent::new(false)),
        }
    }

    fn give_event(&mut self, event: Rc<futures_intrusive::sync::LocalManualResetEvent>) {
        self.events.push_back(event);
    }
}

pub struct TickedTimerFromTimerRegistration {
    timer_system: Rc<RefCell<TimerSystem>>,
}

impl TickedTimerFromTimerRegistration {
    pub fn new(timer_system: Rc<RefCell<TimerSystem>>) -> Self {
        Self { timer_system }
    }

    pub async fn sleep_for(&self, duration: f64) {
        let (event, timer_key) = {
            let mut timer_system_ref_mut = self.timer_system.borrow_mut();
            let event = timer_system_ref_mut.take_event();
            let timer_key = timer_system_ref_mut.add_timer(duration, event.clone());
            (event, timer_key)
        };

        let timer_system = self.timer_system.clone();
        crate::DroppableFuture::new(event.clone().wait(), || {
            let mut timer_system_ref_mut = timer_system.borrow_mut();
            timer_system_ref_mut.remove_timer(timer_key);
            timer_system_ref_mut.give_event(event);
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use crate::TickedAsyncExecutor;

    #[test]
    fn test_timer_registration() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.borrow().events.len(), 0);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);

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
        assert_eq!(timer.timer_system.borrow().events.len(), 1);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);
    }

    #[test]
    fn test_timer_registration2() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.borrow().events.len(), 0);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);

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
        assert_eq!(timer.timer_system.borrow().events.len(), 1);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);

        // Register 2nd timer reusing the same event

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
        assert_eq!(timer.timer_system.borrow().events.len(), 1);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);
    }

    #[test]
    fn test_timer_registration_cancellation_safety() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        assert_eq!(timer.timer_system.borrow().events.len(), 0);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);

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
        assert_eq!(timer.timer_system.borrow().events.len(), 1);
        assert_eq!(timer.timer_system.borrow().timers.len(), 0);
    }
}
