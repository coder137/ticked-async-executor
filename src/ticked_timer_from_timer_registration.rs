use std::sync::mpsc;

pub struct TickedTimerFromTimerRegistration {
    timer_registration_tx: mpsc::Sender<(f64, tokio::sync::oneshot::Sender<()>)>,
}

impl TickedTimerFromTimerRegistration {
    pub fn new(
        timer_registration_tx: mpsc::Sender<(f64, tokio::sync::oneshot::Sender<()>)>,
    ) -> Self {
        Self {
            timer_registration_tx,
        }
    }

    pub async fn sleep_for(&self, duration_in_ms: f64) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ignore = async {
            self.timer_registration_tx
                .send((duration_in_ms, tx))
                .map_err(|_| ())?;
            rx.await.map_err(|_| ())?;
            Ok::<(), ()>(())
        }
        .await;
    }
}
