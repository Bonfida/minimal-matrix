use crate::{error::MatrixClientError, utils::generate_tx_id};
use {
    reqwest::StatusCode,
    serde,
    serde::{Deserialize, Serialize},
    std::sync::Arc,
    std::time::Duration,
    tokio::sync::RwLock,
    tokio::time::Instant,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginResponse {
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginBody {
    #[serde(rename(serialize = "type"))]
    pub _type: String,
    pub password: String,
    pub identifier: Identifier,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Identifier {
    #[serde(rename(serialize = "type"))]
    pub _type: String,
    pub user: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendBody {
    pub msgtype: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendResponseError {
    pub retry_after_ms: u64,
}

#[derive(Clone)]
pub struct MatrixClient {
    pub client: reqwest::Client,
    pub home_server_name: String,
    pub access_token: String,
    pub room_id: String,
    pub sleep_until: Arc<RwLock<Instant>>,
    pub message_q: tokio::sync::mpsc::UnboundedSender<String>,
}

impl MatrixClient {
    pub async fn new(
        home_server_name: String,
        room_id: String,
        access_token: String,
    ) -> Result<Self, MatrixClientError> {
        let (snd, rcv) = tokio::sync::mpsc::unbounded_channel::<String>();
        let client = Self {
            client: reqwest::Client::new(),
            home_server_name,
            room_id,
            access_token,
            sleep_until: Arc::new(RwLock::new(Instant::now())),
            message_q: snd,
        };
        tokio::spawn(run(rcv, client.clone()));
        Ok(client)
    }

    pub async fn _send_message(&mut self, message: String) -> Result<(), MatrixClientError> {
        let body = SendBody {
            msgtype: "m.text".to_string(),
            body: message.clone(),
        };
        let txn_id = generate_tx_id(&message);
        let res = self
            .client
            .put(format!(
                "https://{}.ems.host/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
                self.home_server_name, self.room_id, txn_id
            ))
            .bearer_auth(self.access_token.clone())
            .json(&body)
            .send()
            .await?;

        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            let res = res.json::<SendResponseError>().await.unwrap();
            let mut guard = self.sleep_until.write().await;
            *guard = Instant::now()
                .checked_add(Duration::from_millis(res.retry_after_ms))
                .unwrap();
            return Err(MatrixClientError::TooManyRequest);
        }

        Ok(())
    }

    pub fn send_message(&mut self, message: String) -> Result<(), MatrixClientError> {
        self.message_q.send(message).unwrap();
        Ok(())
    }
}

pub async fn run(
    mut rcv: tokio::sync::mpsc::UnboundedReceiver<String>,
    matrix_client: MatrixClient,
) {
    let mut timed_out = false;
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut messages_q: Vec<String> = vec![];
    let mut retry = 0;

    loop {
        if messages_q.len() > 10 || (timed_out && !messages_q.is_empty()) {
            let deadline = matrix_client.sleep_until.read().await;
            tokio::time::sleep_until(*deadline).await;
            drop(deadline);

            let message = messages_q.join("\n");
            match matrix_client.clone()._send_message(message).await {
                Ok(_) => messages_q.clear(),
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

#[cfg(test)]
#[tokio::test]
async fn test() {
    use dotenv::dotenv;
    use futures::future::join_all;
    use std::env;

    dotenv().ok();
    let home_server_name = env::var("HOME_SERVER_NAME").unwrap();
    let room_id = env::var("ROOM_ID").unwrap();
    let access_token = env::var("ACCESS_TOKEN").unwrap();

    let mut i = 0;
    let mut handles = vec![];
    loop {
        let messages = format!("Test {i}");
        let home_server_name = home_server_name.clone();
        let access_token = access_token.clone();
        let room_id = room_id.clone();
        let h = tokio::spawn(async {
            let mut client = MatrixClient::new(home_server_name, room_id, access_token)
                .await
                .unwrap();
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
