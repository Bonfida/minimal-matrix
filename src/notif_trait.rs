use {async_trait::async_trait, tokio::time::Duration};

use tokio::{sync::RwLockReadGuard, time::Instant};

use crate::error::MatrixClientError;

#[async_trait]
pub trait Notifier {
    async fn _send_message(&mut self, message: String) -> Result<(), MatrixClientError>;
    fn send_message(&self, message: String) -> Result<(), MatrixClientError>;
    async fn get_sleep_until(&self) -> RwLockReadGuard<'_, Instant>;
}

pub async fn run<T: Notifier + Clone>(
    mut rcv: tokio::sync::mpsc::UnboundedReceiver<String>,
    notif_client: T,
) {
    let mut timed_out = false;
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut messages_q: Vec<String> = vec![];
    let mut retry = 0;

    loop {
        if messages_q.len() > 10 || (timed_out && !messages_q.is_empty()) {
            let deadline = notif_client.get_sleep_until().await;
            tokio::time::sleep_until(*deadline).await;
            drop(deadline);

            let message = messages_q.join("\n");
            match notif_client.clone()._send_message(message).await {
                Ok(_) => {
                    messages_q.clear();
                    retry = 0
                }
                Err(MatrixClientError::TooManyRequest) => continue,
                Err(_) => retry += 1,
            }

            if retry == 50 {
                eprintln!(
                    "Reached max retry\n Clearing following messages:\n{:?}",
                    messages_q
                );
                messages_q.clear();
                retry = 0;
            }

            interval.reset();
            timed_out = false;
        }

        if messages_q.len() > 10 {
            // Loop again to empty queue
            continue;
        }

        tokio::select! {
            o = rcv.recv() => {
                if let Some(msg) = o {
                    messages_q.push(msg);
                } else {
                    break;
                }

            }
            _ = interval.tick() => {
                timed_out = true;
                continue
            },
        };
    }
}
