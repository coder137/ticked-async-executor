use std::sync::Arc;

use crate::TaskId;

pub trait TaskObserver {
    fn on_spawn(&self, id: &TaskId);
    fn on_wake(&self, id: &TaskId);
    fn on_tick_start(&self, id: &TaskId, delta: f64);
    fn on_tick_end(&self, id: &TaskId);
    fn on_drop(&self, id: &TaskId);
}

impl TaskObserver for () {
    #[inline(always)]
    fn on_spawn(&self, _id: &TaskId) {}
    #[inline(always)]
    fn on_wake(&self, _id: &TaskId) {}
    #[inline(always)]
    fn on_tick_start(&self, _id: &TaskId, _delta: f64) {}
    #[inline(always)]
    fn on_tick_end(&self, _id: &TaskId) {}
    #[inline(always)]
    fn on_drop(&self, _id: &TaskId) {}
}

impl<O> TaskObserver for Arc<O>
where
    O: TaskObserver,
{
    fn on_spawn(&self, id: &TaskId) {
        (**self).on_spawn(id);
    }
    fn on_wake(&self, id: &TaskId) {
        (**self).on_wake(id);
    }
    fn on_tick_start(&self, id: &TaskId, delta: f64) {
        (**self).on_tick_start(id, delta);
    }
    fn on_tick_end(&self, id: &TaskId) {
        (**self).on_tick_end(id);
    }
    fn on_drop(&self, id: &TaskId) {
        (**self).on_drop(id);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct DefaultTestObserver;
    impl TaskObserver for DefaultTestObserver {
        fn on_spawn(&self, id: &TaskId) {
            println!("on_spawn: {:?}", id);
        }
        fn on_wake(&self, id: &TaskId) {
            println!("on_wake: {:?}", id);
        }
        fn on_tick_start(&self, id: &TaskId, delta: f64) {
            println!("on_tick_start: {:?} {}", id, delta);
        }
        fn on_tick_end(&self, id: &TaskId) {
            println!("on_tick_end: {:?}", id);
        }
        fn on_drop(&self, id: &TaskId) {
            println!("on_drop: {:?}", id);
        }
    }
}
