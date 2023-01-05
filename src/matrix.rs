use crate::{error::MatrixClientError, utils::current_time};
use {
    serde,
    serde::{Deserialize, Serialize},
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

#[derive(Clone)]
pub struct MatrixClient {
    pub client: reqwest::Client,
    pub home_server_name: String,
    pub access_token: String,
    pub room_id: String,
    pub user: String,
    pub password: String,
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

    pub async fn send_message(&self, message: String) -> Result<(), MatrixClientError> {
        let now = current_time();
        let body = SendBody {
            msgtype: "m.text".to_string(),
            body: message,
        };
        self.client
            .put(format!(
                "https://{}.ems.host/_matrix/client/r0/rooms/{}/send/m.room.message/{}",
                self.home_server_name, self.room_id, now
            ))
            .bearer_auth(self.access_token.clone())
            .json(&body)
            .send()
            .await?;
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
    client.send_message("Test".to_string()).await.unwrap();
}
