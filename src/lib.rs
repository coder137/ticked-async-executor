#![doc = include_str!("../README.md")]

mod droppable_future;
use droppable_future::*;

mod task_identifier;
pub use task_identifier::*;

mod split_ticked_async_executor;
pub use split_ticked_async_executor::*;

mod ticked_async_executor;
pub use ticked_async_executor::*;

#[cfg(feature = "tick_event")]
mod ticked_timer_from_tick_event;
#[cfg(feature = "tick_event")]
pub use ticked_timer_from_tick_event::*;

#[cfg(feature = "timer_registration")]
mod ticked_timer_from_timer_registration;
#[cfg(feature = "timer_registration")]
pub use ticked_timer_from_timer_registration::*;
