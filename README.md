# Ticked Async Executor

Rust based Async Executor which executes woken tasks only when it is ticked

# Limitation

- Does not work with the tokio runtime and async constructs that use the tokio runtime internally
