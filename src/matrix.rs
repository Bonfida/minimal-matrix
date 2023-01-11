use crate::{error::MatrixClientError, utils::current_time};
use {
    reqwest::StatusCode,
    serde,
    serde::{Deserialize, Serialize},
    std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    std::time::Duration,
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
    pub user: String,
    pub password: String,
    pub msg_counter: Arc<AtomicU64>,
    pub sleep_until: Instant,
}

impl MatrixClient {
    pub fn new(home_server_name: String, room_id: String, user: String, password: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            home_server_name,
            room_id,
            user,
            password,
            access_token: String::default(),
            msg_counter: Arc::new(AtomicU64::new(0)),
            sleep_until: Instant::now(),
        }
    }

    pub async fn login(&mut self) -> Result<(), MatrixClientError> {
        let access_token = self.get_access_token().await?;
        self.access_token = access_token;
        Ok(())
    }

    pub async fn get_access_token(&self) -> Result<String, MatrixClientError> {
        let body = LoginBody {
            _type: "m.login.password".to_owned(),
            password: self.password.clone(),
            identifier: Identifier {
                _type: "m.id.user".to_owned(),
                user: self.user.clone(),
            },
        };
        let res: LoginResponse = self
            .client
            .post(format!(
                "https://{}.element.io/_matrix/client/v3/login",
                self.home_server_name
            ))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        Ok(res.access_token)
    }

    pub async fn _send_message(&mut self, message: String) -> Result<(), MatrixClientError> {
        let now = current_time();
        let body = SendBody {
            msgtype: "m.text".to_string(),
            body: message.clone(),
        };
        let txn_id =
            (now as u128) << (64 + (self.msg_counter.fetch_add(1, Ordering::Acquire) as u128));
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
            self.sleep_until = Instant::now()
                .checked_add(Duration::from_millis(res.retry_after_ms))
                .unwrap();
            return Err(MatrixClientError::TooManyRequest);
        }

        Ok(())
    }

    pub async fn send_message(&mut self, message: String) -> Result<(), MatrixClientError> {
        let res = self._send_message(message.clone()).await;
        if let Err(MatrixClientError::TooManyRequest) = res {
            // We only retry once if rate limited
            tokio::time::sleep_until(self.sleep_until).await;
            self._send_message(message).await?
        }

        Ok(())
    }
}

#[cfg(test)]
#[tokio::test]
async fn test() {
    use dotenv::dotenv;
    use std::env;
    dotenv().ok();
    let home_server_name = env::var("HOME_SERVER_NAME").unwrap();
    let room_id = env::var("ROOM_ID").unwrap();
    let user = env::var("MATRIX_USER").unwrap();
    let password = env::var("MATRIX_PASSWORD").unwrap();

    let mut client = MatrixClient::new(home_server_name, room_id, user, password);
    client.login().await.unwrap();

    let mut i = 0;
    loop {
        client.send_message("Test".to_string()).await.unwrap();
        i += 1;
        println!("{i}");
    }
}
