//! Transport layer used by [`Client`](crate::Client).
//!
//! The default [`RestTransport`] sends JSON over HTTPS via [`reqwest`].
//! Implement the [`Transport`] trait to plug in a different stack (gRPC,
//! mock, replay, etc.).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest::Method;
use url::Url;

use crate::errors::{ApiError, Error, Result};

/// One outbound API request.
#[derive(Debug, Default, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<serde_json::Value>,
    pub headers: HashMap<String, String>,
    pub stream: bool,
}

/// One transport-level response.
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: ResponseBody,
}

/// Either a buffered response body or a streaming byte stream.
pub enum ResponseBody {
    Bytes(Bytes),
    Stream(Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + Unpin>),
}

impl ResponseBody {
    /// Collect the response body into a `Vec<u8>`, regardless of whether it
    /// arrived buffered or streaming.
    pub async fn collect(self) -> Result<Vec<u8>> {
        match self {
            ResponseBody::Bytes(b) => Ok(b.to_vec()),
            ResponseBody::Stream(mut s) => {
                let mut out = Vec::new();
                while let Some(chunk) = s.next().await {
                    out.extend_from_slice(&chunk?);
                }
                Ok(out)
            }
        }
    }
}

/// Pluggable transport contract.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, request: Request) -> Result<Response>;
}

/// Default JSON-over-HTTPS transport.
#[derive(Clone)]
pub struct RestTransport {
    base_url: Url,
    api_key: String,
    user_agent: String,
    http: reqwest::Client,
}

impl RestTransport {
    pub(crate) fn new(base_url: Url, api_key: String, user_agent: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            base_url,
            api_key,
            user_agent,
            http,
        })
    }

    pub(crate) fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http = client;
        self
    }

    fn build_url(&self, path: &str, query: &[(String, String)]) -> Result<Url> {
        let trimmed = path.trim_start_matches('/');
        let mut url = self.base_url.clone();
        // Append path segments while preserving the base URL path.
        let base_path = url.path().trim_end_matches('/').to_owned();
        let new_path = if base_path.is_empty() {
            format!("/{trimmed}")
        } else {
            format!("{base_path}/{trimmed}")
        };
        url.set_path(&new_path);
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query {
                pairs.append_pair(k, v);
            }
        }
        Ok(url)
    }
}

#[async_trait]
impl Transport for RestTransport {
    async fn send(&self, req: Request) -> Result<Response> {
        let method =
            Method::from_bytes(req.method.as_bytes()).map_err(|e| Error::Other(e.to_string()))?;
        let url = self.build_url(&req.path, &req.query)?;

        let mut builder = self.http.request(method, url);

        let mut headers = HeaderMap::new();
        if !self.user_agent.is_empty() {
            headers.insert(
                USER_AGENT,
                HeaderValue::from_str(&self.user_agent).map_err(|e| Error::Other(e.to_string()))?,
            );
        }
        if !self.api_key.is_empty() {
            let name = HeaderName::from_static("x-api-key");
            headers.insert(
                name,
                HeaderValue::from_str(&self.api_key).map_err(|e| Error::Other(e.to_string()))?,
            );
        }
        if req.body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        if req.stream {
            headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        }
        for (k, v) in &req.headers {
            let name =
                HeaderName::from_bytes(k.as_bytes()).map_err(|e| Error::Other(e.to_string()))?;
            headers.insert(
                name,
                HeaderValue::from_str(v).map_err(|e| Error::Other(e.to_string()))?,
            );
        }
        builder = builder.headers(headers);

        if let Some(body) = req.body {
            builder = builder.json(&body);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let mut header_map = HashMap::with_capacity(resp.headers().len());
        for (k, v) in resp.headers().iter() {
            if let Ok(s) = v.to_str() {
                header_map.insert(k.as_str().to_lowercase(), s.to_owned());
            }
        }

        let body = if req.stream {
            ResponseBody::Stream(Box::new(resp.bytes_stream()))
        } else {
            ResponseBody::Bytes(resp.bytes().await?)
        };

        Ok(Response {
            status,
            headers: header_map,
            body,
        })
    }
}

/// Internal helper used by services to dispatch requests.
pub(crate) struct Dispatcher {
    transport: Arc<dyn Transport>,
}

impl Dispatcher {
    pub(crate) fn new(transport: Arc<dyn Transport>) -> Self {
        Self { transport }
    }

    pub(crate) async fn json<T: serde::de::DeserializeOwned>(&self, req: Request) -> Result<T> {
        let bytes = self.bytes(req).await?;
        if bytes.is_empty() {
            // Some endpoints return empty 2xx bodies.
            return serde_json::from_slice(b"null").map_err(Into::into);
        }
        serde_json::from_slice(&bytes).map_err(Into::into)
    }

    pub(crate) async fn bytes(&self, req: Request) -> Result<Vec<u8>> {
        let resp = self.transport.send(req).await?;
        ensure_success(&resp)?;
        let bytes = match resp.body {
            ResponseBody::Bytes(b) => b.to_vec(),
            ResponseBody::Stream(mut s) => {
                let mut out = Vec::new();
                while let Some(chunk) = s.next().await {
                    out.extend_from_slice(&chunk?);
                }
                out
            }
        };
        Ok(bytes)
    }

    pub(crate) async fn empty(&self, req: Request) -> Result<()> {
        let resp = self.transport.send(req).await?;
        ensure_success(&resp)?;
        // Drain the body so connections can be reused.
        let _ = resp.body.collect().await?;
        Ok(())
    }

    pub(crate) async fn stream(
        &self,
        req: Request,
    ) -> Result<Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + Unpin>>
    {
        let resp = self.transport.send(req).await?;
        if !(200..300).contains(&resp.status) {
            let body = resp.body.collect().await?;
            return Err(Error::Api(ApiError::from_parts(
                resp.status,
                resp.headers,
                body,
            )));
        }
        match resp.body {
            ResponseBody::Stream(s) => Ok(s),
            ResponseBody::Bytes(b) => {
                // Wrap a buffered response in a single-shot stream so callers
                // can use the same parsing path.
                let stream = futures_util::stream::iter(vec![Ok(b)]);
                Ok(Box::new(Box::pin(stream)))
            }
        }
    }
}

fn ensure_success(resp: &Response) -> Result<()> {
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        // Body-bearing responses are read to extract the error message; for
        // streamed responses callers must use Dispatcher::stream which handles
        // the body separately.
        if let ResponseBody::Bytes(b) = &resp.body {
            return Err(Error::Api(ApiError::from_parts(
                resp.status,
                resp.headers.clone(),
                b.to_vec(),
            )));
        }
        Err(Error::Api(ApiError::from_parts(
            resp.status,
            resp.headers.clone(),
            Vec::new(),
        )))
    }
}
