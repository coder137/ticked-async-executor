pub struct TickedTimerFromTickEvent {
    tick_event_rx: tokio::sync::watch::Receiver<f64>,
}

impl TickedTimerFromTickEvent {
    pub fn new(tick_event_rx: tokio::sync::watch::Receiver<f64>) -> Self {
        Self { tick_event_rx }
    }

    pub async fn sleep_for(mut self, mut duration_in_ms: f64) {
        loop {
            let _r = self.tick_event_rx.changed().await;
            if _r.is_err() {
                // This means that the executor supplying the delta channel has shutdown
                // We must stop waiting gracefully
                break;
            }
            let current_dt = *self.tick_event_rx.borrow_and_update();
            duration_in_ms -= current_dt;
            if duration_in_ms <= 0.0 {
                break;
            }
        }
    }
}
