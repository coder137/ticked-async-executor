use ticked_async_executor::TickedAsyncExecutor;

const DELTA: f64 = 1000.0 / 60.0;

#[test]
fn test_tokio_join() {
    let executor = TickedAsyncExecutor::default();

    let (tx1, mut rx1) = tokio::sync::mpsc::channel::<usize>(1);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel::<usize>(1);
    executor
        .spawn("ThreadedFuture", async move {
            let (a, b) = tokio::join!(rx1.recv(), rx2.recv());
            assert_eq!(a.unwrap(), 10);
            assert_eq!(b.unwrap(), 20);
        })
        .detach();

    let (tx3, mut rx3) = tokio::sync::mpsc::channel::<usize>(1);
    let (tx4, mut rx4) = tokio::sync::mpsc::channel::<usize>(1);
    executor
        .spawn("LocalFuture", async move {
            let (a, b) = tokio::join!(rx3.recv(), rx4.recv());
            assert_eq!(a.unwrap(), 10);
            assert_eq!(b.unwrap(), 20);
        })
        .detach();

    tx1.try_send(10).unwrap();
    tx3.try_send(10).unwrap();
    for _ in 0..10 {
        executor.tick(DELTA);
    }
    tx2.try_send(20).unwrap();
    tx4.try_send(20).unwrap();

    while executor.num_tasks() != 0 {
        executor.tick(DELTA);
    }
}

#[test]
fn test_tokio_select() {
    let executor = TickedAsyncExecutor::default();

    let (tx1, mut rx1) = tokio::sync::mpsc::channel::<usize>(1);
    let (_tx2, mut rx2) = tokio::sync::mpsc::channel::<usize>(1);
    executor
        .spawn("ThreadedFuture", async move {
            tokio::select! {
                data = rx1.recv() => {
                    assert_eq!(data.unwrap(), 10);
                }
                _ = rx2.recv() => {}
            }
        })
        .detach();

    let (tx3, mut rx3) = tokio::sync::mpsc::channel::<usize>(1);
    let (_tx4, mut rx4) = tokio::sync::mpsc::channel::<usize>(1);
    executor
        .spawn("LocalFuture", async move {
            tokio::select! {
                data = rx3.recv() => {
                    assert_eq!(data.unwrap(), 10);
                }
                _ = rx4.recv() => {}
            }
        })
        .detach();

    for _ in 0..10 {
        executor.tick(DELTA);
    }

    tx1.try_send(10).unwrap();
    tx3.try_send(10).unwrap();
    while executor.num_tasks() != 0 {
        executor.tick(DELTA);
    }
}
