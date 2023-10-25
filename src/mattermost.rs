use crate::error::MatrixClientError;
use serde::Serialize;

pub const ICON: &str = "https://mattermost.com/wp-content/uploads/2022/02/icon.png";
pub const USERNAME: &str = "bot";

pub struct MatterMost {
    pub client: reqwest::Client,
    pub url: String,
}

#[derive(Serialize)]
pub struct Message {
    text: String,
    username: String,
    icon_url: String,
}

impl MatterMost {
    pub fn new(url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_owned(),
        }
    }

    pub async fn send_message(&self, message: &str) -> Result<(), MatrixClientError> {
        let message: Message = Message {
            text: message.to_owned(),
            username: USERNAME.to_owned(),
            icon_url: ICON.to_owned(),
        };

        self.client.post(&self.url).json(&message).send().await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use dotenv::dotenv;
    use std::env::var;

    #[tokio::test]
    async fn test_send_message() {
        dotenv().ok();

        let client = MatterMost::new(&var("MATTER_MOST").unwrap());
        client.send_message("Test client").await.unwrap();
    }
}
