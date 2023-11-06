use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum MatrixClientError {
    Reqwest(reqwest::Error),
    TooManyRequest,
    HeaderParsing,
    Parsing,
}

impl From<reqwest::Error> for MatrixClientError {
    fn from(e: reqwest::Error) -> MatrixClientError {
        Self::Reqwest(e)
    }
}
