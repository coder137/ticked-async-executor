use std::collections::VecDeque;

use criterion::{Criterion, criterion_group, criterion_main};

use ticked_async_executor::TickedAsyncExecutor;

fn ticked_async_executor_benchmark(c: &mut Criterion) {
    spawn_tasks_benchmark(c);

    #[cfg(feature = "tick_event")]
    timer_from_tick_event_benchmark(c);

    #[cfg(feature = "timer_registration")]
    timer_from_timer_registration_benchmark(c);
}

fn spawn_tasks_benchmark(c: &mut Criterion) {
    c.bench_function("Spawn 1 task", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            executor.spawn_local("empty", async move {}).detach();
            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 2 tasks", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            executor.spawn_local("empty1", async move {}).detach();
            executor.spawn_local("empty2", async move {}).detach();

            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 100 tasks", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            for _ in 0..100 {
                executor.spawn_local("_", async move {}).detach();
            }

            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 1000 tasks", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            for _ in 0..1000 {
                executor.spawn_local("_", async move {}).detach();
            }

            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 10000 tasks", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();
            for _ in 0..10000 {
                executor.spawn_local("_", async move {}).detach();
            }

            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    enum T {
        Start(usize, tokio::sync::oneshot::Sender<usize>),
        Mid(
            usize,
            tokio::sync::oneshot::Receiver<usize>,
            tokio::sync::oneshot::Sender<usize>,
        ),
        End(usize, tokio::sync::oneshot::Receiver<usize>),
    }

    c.bench_function("A -> B -> C .. 1000", move |b| {
        let mut executor = TickedAsyncExecutor::default();
        b.iter_with_large_drop(move || {
            const CAPACITY: usize = 1000;
            let mut v = Vec::with_capacity(CAPACITY);
            let mut queues = VecDeque::with_capacity(CAPACITY);
            for i in 0..CAPACITY {
                let (tx1, rx1) = tokio::sync::oneshot::channel::<usize>();
                match queues.pop_front() {
                    Some(stored_rx) => v.push(T::Mid(i, stored_rx, tx1)),
                    None => {
                        v.push(T::Start(i, tx1));
                    }
                }
                queues.push_back(rx1);
            }
            let stored_rx = queues.pop_front().unwrap();
            v.push(T::End(CAPACITY, stored_rx));

            for t in v.drain(..) {
                match t {
                    T::Start(i, sender) => {
                        executor
                            .spawn_local("_", async move {
                                sender.send(i).unwrap();
                            })
                            .detach();
                    }
                    T::Mid(i, receiver, sender) => {
                        executor
                            .spawn_local("_", async move {
                                let v = receiver.await.unwrap();
                                assert_eq!(v, i - 1);
                                sender.send(i).unwrap();
                            })
                            .detach();
                    }
                    T::End(i, receiver) => {
                        executor
                            .spawn_local("_", async move {
                                let v = receiver.await.unwrap();
                                assert_eq!(v, i - 1);
                            })
                            .detach();
                    }
                }
            }

            executor.tick_all(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });
}

#[cfg(feature = "tick_event")]
fn timer_from_tick_event_benchmark(c: &mut Criterion) {
    c.bench_function("Spawn 1 timer from tick event", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();

            let timer = executor.create_timer_from_tick_event();
            executor
                .spawn_local("empty", async move {
                    timer.sleep_for(1.0).await.unwrap_or_default();
                })
                .detach();

            executor.wait_till_completed(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 1000 timers from tick event", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();

            for _ in 0..1000 {
                let timer = executor.create_timer_from_tick_event();
                executor
                    .spawn_local("empty", async move {
                        timer.sleep_for(1.0).await.unwrap_or_default();
                    })
                    .detach();
            }

            executor.wait_till_completed(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });
}

#[cfg(feature = "timer_registration")]
fn timer_from_timer_registration_benchmark(c: &mut Criterion) {
    c.bench_function("Spawn 1 timer from timer registration", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();

            let timer = executor.create_timer_from_timer_registration();
            executor
                .spawn_local("empty", async move {
                    timer.sleep_for(1.0).await;
                })
                .detach();

            executor.wait_till_completed(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });

    c.bench_function("Spawn 1000 timers from timer registration", |b| {
        b.iter_with_large_drop(|| {
            let mut executor = TickedAsyncExecutor::default();

            for _ in 0..1000 {
                let timer = executor.create_timer_from_timer_registration();
                executor
                    .spawn_local("empty", async move {
                        timer.sleep_for(1.0).await;
                    })
                    .detach();
            }

            executor.wait_till_completed(0.1);
            assert_eq!(executor.num_tasks(), 0);
        });
    });
}

criterion_group!(benches, ticked_async_executor_benchmark);
criterion_main!(benches);
