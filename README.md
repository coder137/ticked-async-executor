# Ticked Async Executor

Rust based Async Executor which executes woken tasks only when it is ticked

# Example

```rust
let executor = TickedAsyncExecutor::default();

executor.spawn_local("MyIdentifier", async move {}).detach();

// Make sure to tick your executor to run the tasks
executor.tick(DELTA, LIMIT);
```

# Limitation

- Does not work with the tokio runtime and async constructs that use the tokio runtime internally
