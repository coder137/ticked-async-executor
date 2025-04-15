# Ticked Async Executor

Async Local Executor which executes woken tasks only when it is ticked

# Usage

## Default Local Executor

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;

let executor = TickedAsyncExecutor::default();

executor.spawn_local("MyIdentifier", async move {}).detach();

// Make sure to tick your executor to run the tasks
executor.tick(DELTA, None);
```

## Split Local Executor

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;
let task_state_cb: fn(TaskState) = |_state| {};

let (spawner, ticker) = new_split_ticked_async_executor(task_state_cb);

spawner.spawn_local("MyIdentifier", async move {}).detach();

// Tick your ticker to run the tasks
ticker.tick(DELTA, None);
```

## Limit the number of woken tasks run per tick

```rust
use ticked_async_executor::*;

const DELTA: f64 = 1000.0 / 60.0;

let executor = TickedAsyncExecutor::default();

executor.spawn_local("MyIdentifier", async move {}).detach();

// Runs upto 10 woken tasks per tick
executor.tick(DELTA, Some(10));
```

# Caveats

- Uses the `smol` ecosystem
- Ensure that tasks are spawned on the same thread as the one that initializes the executor
