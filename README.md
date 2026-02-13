# Ticked Async Executor

Async Local Executor which executes woken tasks only when it is ticked

# Usage

## Default Local Executor

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;

let mut executor = TickedAsyncExecutor::default();

executor.spawn_local("MyIdentifier", async move {}).detach();

// Tick your executor to run the tasks
assert_eq!(executor.num_tasks(), 1);
executor.tick(DELTA, None);
assert_eq!(executor.num_tasks(), 0);
```

## Split Local Executor

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;

let (spawner, mut ticker) = SplitTickedAsyncExecutor::default();

spawner.spawn_local("MyIdentifier", async move {}).detach();

// Tick your ticker to run the tasks
assert_eq!(spawner.num_tasks(), 1);
ticker.tick(DELTA, None);
assert_eq!(spawner.num_tasks(), 0);
```

## Limit the number of woken tasks run per tick

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;

let mut executor = TickedAsyncExecutor::default();

executor.spawn_local("MyIdentifier1", async move {}).detach();
executor.spawn_local("MyIdentifier2", async move {}).detach();

// Runs upto 1 woken tasks per tick
assert_eq!(executor.num_tasks(), 2);
executor.tick(DELTA, Some(1));
assert_eq!(executor.num_tasks(), 1);
executor.tick(DELTA, Some(1));
assert_eq!(executor.num_tasks(), 0);
```

# Features

- `tick_event`
- `timer_registration`

## Tick Registration

```rust
#[cfg(feature = "timer_registration")]
{
  use ticked_async_executor::*;

  const DELTA: f64 = 1000.0 / 60.0;

  let mut executor = TickedAsyncExecutor::default();

  // These APIs are gated under the `timer_registration` feature
  let timer = executor.create_timer_from_timer_registration();

  executor.spawn_local("MyIdentifier1", async move {
    timer.sleep_for(10.0).await;
  }).detach();
  
  executor.wait_till_completed(1.0);
  assert_eq!(executor.num_tasks(), 0);
}
```

## Tick Event

```rust
#[cfg(feature = "tick_event")]
{
  use ticked_async_executor::*;

  const DELTA: f64 = 1000.0 / 60.0;

  let mut executor = TickedAsyncExecutor::default();

  // These APIs are gated under the `tick_event` feature
  let _delta_tick_rx = executor.tick_channel();
  let timer = executor.create_timer_from_tick_event();

  executor.spawn_local("MyIdentifier1", async move {
    timer.sleep_for(10.0).await;
  }).detach();

  executor.wait_till_completed(1.0);
  assert_eq!(executor.num_tasks(), 0);
}
```

# Benchmarks

- `executor.spawn_local`
```text
Spawn 10000 tasks
time:   [1.3711 ms 1.3713 ms 1.3715 ms]
```

- `executor.create_timer_from_timer_registration` under feature `timer_registration`
```text
Spawn 1000 timers from timer registration
time:   [336.10 µs 336.42 µs 336.93 µs]
```

- `executor.create_timer_from_tick_event` under feature `tick_event`
```text
Spawn 1000 timers from tick event
time:   [1.5688 ms 1.5692 ms 1.5697 ms]
```

# Caveats

- Uses the `smol` ecosystem
- Ensure that tasks are spawned on the same thread as the one that initializes the executor

# Roadmap

- [x] TickedAsyncExecutor
- [x] SplitTickedAsyncExecutor
  - Similar to the channel API, but spawner and ticker cannot be moved to different threads 
