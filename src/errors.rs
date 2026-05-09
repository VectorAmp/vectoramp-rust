//! Error types returned by the SDK.

use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

/// Convenience [`Result`] alias used by the SDK.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors surfaced by the SDK.
#[derive(Debug, Error)]
pub enum Error {
    /// The API responded with a non-2xx status.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// The HTTP transport itself failed.
    #[error("vectoramp: http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON encoding or decoding failed.
    #[error("vectoramp: json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The supplied URL was invalid.
    #[error("vectoramp: invalid url: {0}")]
    Url(#[from] url::ParseError),

    /// A local file operation failed.
    #[error("vectoramp: io error: {0}")]
    Io(#[from] std::io::Error),

    /// Caller supplied invalid input to an SDK helper.
    #[error("vectoramp: invalid input: {0}")]
    InvalidInput(String),

    /// A streaming response could not be parsed.
    #[error("vectoramp: stream error: {0}")]
    Stream(String),

    /// Catch-all for other error conditions.
    #[error("vectoramp: {0}")]
    Other(String),
}

impl Error {
    /// Construct an [`Error::InvalidInput`] from any displayable value.
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Error::InvalidInput(msg.into())
    }
}

/// API error returned when the server responds with a non-2xx status.
#[derive(Debug, Error)]
#[error("vectoramp: api error {status}: {message}")]
pub struct ApiError {
    /// HTTP status code returned by the API.
    pub status: u16,
    /// Best-effort human readable message extracted from the body.
    pub message: String,
    /// Raw response body bytes.
    pub body: Vec<u8>,
    /// Response headers, lowercased.
    pub headers: HashMap<String, String>,
}

impl ApiError {
    pub(crate) fn from_parts(status: u16, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        let message = extract_message(&body).unwrap_or_else(|| {
            String::from_utf8_lossy(&body)
                .trim()
                .chars()
                .take(512)
                .collect()
        });
        ApiError {
            status,
            message,
            body,
            headers,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    error: Option<String>,
    message: Option<String>,
    detail: Option<String>,
}

fn extract_message(body: &[u8]) -> Option<String> {
    let parsed: ErrorBody = serde_json::from_slice(body).ok()?;
    parsed.error.or(parsed.message).or(parsed.detail)
}
