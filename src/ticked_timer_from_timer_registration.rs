use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Default)]
pub struct TimerSystem {
    events: RefCell<VecDeque<Rc<futures_intrusive::sync::LocalManualResetEvent>>>,
    timers: RefCell<Vec<(f64, Rc<futures_intrusive::sync::LocalManualResetEvent>)>>,
}

impl TimerSystem {
    pub async fn register_sleep_for(&self, duration: f64) {
        let event = self.take_event();
        self.timers.borrow_mut().push((duration, event.clone()));
        event.wait().await;
        self.give_event(event);
    }

    pub fn resolve_timers(&self, delta: f64) {
        self.timers
            .borrow_mut()
            .extract_if(.., |(elapsed, _)| {
                *elapsed -= delta;
                *elapsed <= 0.0
            })
            .for_each(|(_, event)| {
                event.set();
            });
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
        self.timer_system.register_sleep_for(duration).await
    }
}

#[cfg(test)]
mod tests {
    use crate::TickedAsyncExecutor;

    #[test]
    fn test_timer_registration_success() {
        let mut executor = TickedAsyncExecutor::default();

        let timer = executor.create_timer_from_timer_registration();
        executor
            .spawn_local((), async move {
                println!("TASK BEFORE");
                timer.sleep_for(10.0).await;
                println!("TASK AFTER");
            })
            .detach();

        for i in 0..=10 {
            println!("TICK BEFORE: {i}");
            executor.tick_all(1.0);
            println!("TICK AFTER: {i}");
        }
        assert_eq!(executor.num_tasks(), 0);
    }
}
