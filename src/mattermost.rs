use crate::{
    error::MatrixClientError,
    notif_trait::{run, Notifier},
};

use {
    async_trait::async_trait,
    serde::Serialize,
    std::{sync::Arc, time::Duration},
    tokio::{
        sync::{RwLock, RwLockReadGuard},
        time::Instant,
    },
};

pub const ICON: &str = "https://mattermost.com/wp-content/uploads/2022/02/icon.png";
pub const USERNAME: &str = "bot";

#[derive(Clone, Debug)]
pub struct MatterMost {
    pub client: reqwest::Client,
    pub url: String,
    pub sleep_until: Arc<RwLock<Instant>>,
    pub message_q: tokio::sync::mpsc::UnboundedSender<String>,
    #[cfg(feature = "log")]
    pub encoder: log4rs::encode::pattern::PatternEncoder,
}

#[derive(Serialize)]
pub struct Message {
    text: String,
    username: String,
    icon_url: String,
}

impl MatterMost {
    pub fn new(url: &str) -> Self {
        let (snd, rcv) = tokio::sync::mpsc::unbounded_channel::<String>();
        #[cfg(feature = "log")]
        let encoder = log4rs::encode::pattern::PatternEncoder::new(
            "`{d(%Y-%m-%d %H:%M:%S %Z)(utc)}: {l:<5} {f}:{L} - {m} {K(inner_error)( )}{K(tx_signature)( )}{K(trace)}{n}`",
        );
        let client = Self {
            client: reqwest::Client::new(),
            url: url.to_owned(),
            message_q: snd,
            sleep_until: Arc::new(RwLock::new(Instant::now())),
            #[cfg(feature = "log")]
            encoder,
        };
        tokio::spawn(run(rcv, client.clone()));
        client
    }
}

#[async_trait]
impl Notifier for MatterMost {
    async fn _send_message(&mut self, message: String) -> Result<(), MatrixClientError> {
        let message: Message = Message {
            text: message.to_owned(),
            username: USERNAME.to_owned(),
            icon_url: ICON.to_owned(),
        };

        let res = self.client.post(&self.url).json(&message).send().await?;

        if let Some(limit_remaining) = res.headers().get("X-Ratelimit-Remaining") {
            if limit_remaining == "0" {
                if let Some(limit_reset) = res.headers().get("X-Ratelimit-Reset") {
                    let sleep_duration = limit_reset
                        .to_str()
                        .map_err(|_| MatrixClientError::HeaderParsing)?
                        .parse::<u64>()
                        .map_err(|_| MatrixClientError::Parsing)?;
                    *self.sleep_until.write().await =
                        Instant::now() + Duration::from_secs(sleep_duration + 1);
                }
            }
        }

        Ok(())
    }

    fn send_message(&self, message: String) -> Result<(), MatrixClientError> {
        self.message_q.send(message).unwrap();
        Ok(())
    }

    async fn get_sleep_until(&self) -> RwLockReadGuard<'_, Instant> {
        self.sleep_until.read().await
    }
}

#[cfg(feature = "log")]
impl log4rs::append::Append for MatterMost {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        use log4rs::encode::Encode;
        let mut message_buffer: Vec<u8> = Vec::new();
        let mut writer = log4rs::encode::writer::simple::SimpleWriter(&mut message_buffer);
        self.encoder.encode(&mut writer, record)?;
        let message = String::from_utf8(message_buffer)?;
        self.send_message(message)?;
        Ok(())
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod test {
    use super::*;
    use dotenv::dotenv;
    use futures::future::join_all;
    use std::{env::var, time::Duration};

    #[tokio::test]
    async fn test_send_message() {
        dotenv().ok();

        let mut i = 0;
        let mut handles = vec![];
        loop {
            let messages = format!("Test {i}");
            let h = tokio::spawn(async {
                let client = MatterMost::new(&var("MATTER_MOST").unwrap());
                client.send_message(messages).unwrap();
            });
            handles.push(h);

            i += 1;

            if i == 10 {
                break;
            }
        }

        join_all(handles).await;
        tokio::time::sleep(Duration::from_secs(100)).await;
    }

    #[tokio::test]
    async fn test_send_batch() {
        dotenv().ok();

        let mut i = 0;
        let client = MatterMost::new(&var("MATTER_MOST").unwrap());
        loop {
            let message = format!("Test {i}");
            client.send_message(message).unwrap();
            i += 1;

            if i == 10 {
                break;
            }
        }

        tokio::time::sleep(Duration::from_secs(100)).await;
    }
}
