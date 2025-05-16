use criterion::{criterion_group, criterion_main, Criterion};

use ticked_async_executor::TickedAsyncExecutor;

fn spawn_tasks_benchmark(c: &mut Criterion) {
    c.bench_function("1 task", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            executor.spawn_local("empty", async move {}).detach();
            executor.tick(0.1, None);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("2 tasks", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            executor.spawn_local("empty1", async move {}).detach();
            executor.spawn_local("empty2", async move {}).detach();

            executor.tick(0.1, None);
            assert_eq!(executor.num_tasks(), 0);
        });
    });
}

criterion_group!(benches, spawn_tasks_benchmark);
criterion_main!(benches);
