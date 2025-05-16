# Ticked Async Executor

Async Local Executor which executes woken tasks only when it is ticked

MSRV: 1.87

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

# Caveats

- Uses the `smol` ecosystem
- Ensure that tasks are spawned on the same thread as the one that initializes the executor

# Roadmap

- [x] TickedAsyncExecutor
- [x] SplitTickedAsyncExecutor
  - Similar to the channel API, but spawner and ticker cannot be moved to different threads 
- [ ] Tracing
