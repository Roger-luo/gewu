use std::io::Cursor;

use serde::{Serialize, de::DeserializeOwned};
use tiny_http::{Header, Request, Response, StatusCode};

pub(crate) type Reply = Response<Cursor<Vec<u8>>>;

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

/// JSON replies, each closing its connection.
pub(crate) struct Json;

impl Json {
    pub(crate) fn ok<T: Serialize>(value: T) -> Reply {
        match serde_json::to_vec(&value) {
            Ok(bytes) => Self::reply(bytes, 200),
            Err(error) => Self::error(500, &error.to_string()),
        }
    }

    pub(crate) fn error(status: u16, message: &str) -> Reply {
        let body = serde_json::to_vec(&ErrorBody { error: message })
            .unwrap_or_else(|_| br#"{"error":"internal error"}"#.to_vec());
        Self::reply(body, status)
    }

    /// The request body as `T`, or the reply explaining why it is not one.
    pub(crate) fn body<T: DeserializeOwned>(request: &mut Request) -> Result<T, Reply> {
        let mut body = Vec::new();
        request
            .as_reader()
            .read_to_end(&mut body)
            .map_err(|error| Self::error(400, &error.to_string()))?;
        serde_json::from_slice(&body).map_err(|error| Self::error(400, &error.to_string()))
    }

    fn reply(bytes: Vec<u8>, status: u16) -> Reply {
        Response::from_data(bytes)
            .with_header(header("Content-Type", "application/json; charset=utf-8"))
            .with_header(header("Connection", "close"))
            .with_status_code(StatusCode(status))
    }
}

pub(crate) fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header bytes are ASCII")
}
