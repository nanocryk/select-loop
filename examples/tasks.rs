use {
    futures::Future,
    select_loop::select_loop,
    std::time::Duration,
    tokio::sync::{mpsc, oneshot},
    tokio_stream::wrappers::ReceiverStream,
};

#[tokio::main]
async fn main() {
    let (mut request_tx, request_rx) = mpsc::channel(100);
    tokio::spawn(server(request_rx, 4));

    // we create enough requests to hit the limit.
    let r1 = request(&mut request_tx, 10).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let r2 = request(&mut request_tx, 20).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let r3 = request(&mut request_tx, 30).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let r4 = request(&mut request_tx, 40).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    // this one will be rejected
    let r5 = request(&mut request_tx, 50).await;

    let mut counter = 0;

    select_loop! {
        @after => counter += 1,
        F r5 => |x| assert_eq!((counter, x), (0, None)),
        F r1 => |x| {
            assert_eq!((counter, x), (1, Some(11)));
            // one new slot should be available
            let r6 = request(&mut request_tx, 60).await;
            // then no slots are available
            let r7 = request(&mut request_tx, 70).await;
            assert_eq!(r7.await, None);
            assert_eq!(r6.await, Some(61));
        },
        F r2 => |x| assert_eq!((counter, x), (2, Some(21))),
        F r3 => |x| assert_eq!((counter, x), (3, Some(31))),
        F r4 => |x| assert_eq!((counter, x), (4, Some(41))),
        @exhausted => {
            assert_eq!(counter, 5);
            println!("Exhausted!");
        },
    }
}

struct Request {
    data: u32,
    respond_to: oneshot::Sender<Option<u32>>,
}

/// First .await will send the request, while second will receive the response.
async fn request(
    request_tx: &mut mpsc::Sender<Request>,
    data: u32,
) -> impl Future<Output = Option<u32>> + Unpin {
    let (response_tx, response_rx) = oneshot::channel();
    let _ = request_tx
        .send(Request {
            data,
            respond_to: response_tx,
        })
        .await;

    Box::pin(async move { response_rx.await.unwrap_or(None) })
}

/// Function simulating a server answering requests taking some time to respond.
/// Take a limit of how many requests can be processed, the server will refuse
/// requests if it goes over the limit by responding None.
async fn server(requests_rx: mpsc::Receiver<Request>, limit: u32) {
    let mut currently_processing = 0;
    let requests_rx = ReceiverStream::new(requests_rx);

    let (done_tx, done_rx) = mpsc::channel::<()>(100);
    let done_rx = ReceiverStream::new(done_rx);

    select_loop! {
        S requests_rx => |request| {
            // We respond None if we're over the limit.
            if currently_processing == limit {
                let _ = request.respond_to.send(None);
                continue;
            }

            currently_processing += 1;
            let done_tx = done_tx.clone();

            // Spawn a new task that will wait, send response and notify back that
            // the task is done.
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = request.respond_to.send(Some(request.data + 1));
                done_tx.send(()).await
            });
        },
        S done_rx => |_| currently_processing -= 1,
    }
}
