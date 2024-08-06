use futures::StreamExt;
use ticked_async_executor::TickedAsyncExecutor;

const DELTA: f64 = 1000.0 / 60.0;

// #[test]
// fn test_futures_join() {
//     let executor = TickedAsyncExecutor::default();

//     let (mut tx1, mut rx1) = futures::channel::mpsc::channel::<usize>(1);
//     let (mut tx2, mut rx2) = futures::channel::mpsc::channel::<usize>(1);
//     executor
//         .spawn_local("ThreadedFuture", async move {
//             let (a, b) = futures::join!(rx1.next(), rx2.next());
//             assert_eq!(a.unwrap(), 10);
//             assert_eq!(b.unwrap(), 20);
//         })
//         .detach();

//     let (mut tx3, mut rx3) = futures::channel::mpsc::channel::<usize>(1);
//     let (mut tx4, mut rx4) = futures::channel::mpsc::channel::<usize>(1);
//     executor
//         .spawn_local("LocalFuture", async move {
//             let (a, b) = futures::join!(rx3.next(), rx4.next());
//             assert_eq!(a.unwrap(), 10);
//             assert_eq!(b.unwrap(), 20);
//         })
//         .detach();

//     tx1.try_send(10).unwrap();
//     tx3.try_send(10).unwrap();
//     for _ in 0..10 {
//         executor.tick(DELTA);
//     }
//     tx2.try_send(20).unwrap();
//     tx4.try_send(20).unwrap();

//     while executor.num_tasks() != 0 {
//         executor.tick(DELTA);
//     }
// }

// #[test]
// fn test_futures_select() {
//     let executor = TickedAsyncExecutor::default();

//     let (mut tx1, mut rx1) = futures::channel::mpsc::channel::<usize>(1);
//     let (_tx2, mut rx2) = futures::channel::mpsc::channel::<usize>(1);
//     executor
//         .spawn_local("ThreadedFuture", async move {
//             futures::select! {
//                 data = rx1.next() => {
//                     assert_eq!(data.unwrap(), 10);
//                 }
//                 _ = rx2.next() => {}
//             }
//         })
//         .detach();

//     let (mut tx3, mut rx3) = futures::channel::mpsc::channel::<usize>(1);
//     let (_tx4, mut rx4) = futures::channel::mpsc::channel::<usize>(1);
//     executor
//         .spawn_local("LocalFuture", async move {
//             futures::select! {
//                 data = rx3.next() => {
//                     assert_eq!(data.unwrap(), 10);
//                 }
//                 _ = rx4.next() => {}
//             }
//         })
//         .detach();

//     for _ in 0..10 {
//         executor.tick(DELTA);
//     }

//     tx1.try_send(10).unwrap();
//     tx3.try_send(10).unwrap();
//     while executor.num_tasks() != 0 {
//         executor.tick(DELTA);
//     }
// }
