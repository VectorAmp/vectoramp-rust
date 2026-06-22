//! Intelligence service: non-streaming and streaming RAG queries.

use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;

use crate::client::Client;
use crate::datasets::{push_pagination, Pagination};
use crate::errors::{Error, Result};
use crate::transport::Request;
use crate::types::{
    AppendMessageRequest, AskRequest, AskResponse, ConversationMessage, CreateSessionRequest,
    IntelligenceSession, MessageList, Metadata, SessionList, SessionMessage,
};

/// Optional knobs passed to [`IntelligenceService::ask_with`].
#[derive(Debug, Default, Clone)]
pub struct AskOptions {
    /// Dataset id (string), `"all"` for cross-dataset queries, or `None`.
    pub dataset_id: Option<Value>,
    pub top_k: Option<u32>,
    pub conversation_history: Option<Vec<ConversationMessage>>,
    pub include_sources: Option<bool>,
}

impl AskOptions {
    /// Scope the query to one dataset id.
    pub fn with_dataset<S: Into<String>>(mut self, dataset_id: S) -> Self {
        self.dataset_id = Some(Value::String(dataset_id.into()));
        self
    }

    /// Scope the query across all accessible datasets.
    pub fn with_all_datasets(mut self) -> Self {
        self.dataset_id = Some(Value::String("all".into()));
        self
    }

    /// Set the retrieval `top_k`.
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Toggle source citations.
    pub fn with_sources(mut self, include: bool) -> Self {
        self.include_sources = Some(include);
        self
    }

    /// Attach conversation history.
    pub fn with_history(mut self, history: Vec<ConversationMessage>) -> Self {
        self.conversation_history = Some(history);
        self
    }
}

/// Service running retrieval-augmented question answering.
#[derive(Clone)]
pub struct IntelligenceService {
    client: Client,
}

impl IntelligenceService {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// Run a non-streaming intelligence query.
    pub async fn ask<S: Into<String>>(&self, query: S) -> Result<AskResponse> {
        self.ask_with(query.into(), AskOptions::default()).await
    }

    /// Run a non-streaming intelligence query with explicit options.
    pub async fn ask_with<S: Into<String>>(
        &self,
        query: S,
        options: AskOptions,
    ) -> Result<AskResponse> {
        let request = AskRequest {
            query: query.into(),
            dataset_id: options.dataset_id,
            top_k: options.top_k,
            conversation_history: options.conversation_history,
            stream: false,
            include_sources: options.include_sources,
        };
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/intelligence/query".into(),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Open a streaming intelligence query.
    ///
    /// The returned [`AskStream`] yields decoded server-sent events. The SDK
    /// always sets `stream: true` on the request body.
    pub async fn stream<S: Into<String>>(
        &self,
        query: S,
        options: AskOptions,
    ) -> Result<AskStream> {
        let request = AskRequest {
            query: query.into(),
            dataset_id: options.dataset_id,
            top_k: options.top_k,
            conversation_history: options.conversation_history,
            stream: true,
            include_sources: options.include_sources,
        };
        let body = serde_json::to_value(&request)?;
        let stream = self
            .client
            .dispatcher()
            .stream(Request {
                method: "POST".into(),
                path: "/intelligence/query".into(),
                body: Some(body),
                stream: true,
                ..Default::default()
            })
            .await?;
        Ok(AskStream::new(stream))
    }

    /// Create a new intelligence session.
    ///
    /// Pass `()` for an empty session, or a [`CreateSessionRequest`] (also
    /// accepts a bare title via `&str`/`String`) to set a title, dataset scope,
    /// or metadata.
    pub async fn create_session<R: Into<CreateSessionRequest>>(
        &self,
        request: R,
    ) -> Result<IntelligenceSession> {
        let body = serde_json::to_value(request.into())?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/intelligence/sessions".into(),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// List intelligence sessions.
    ///
    /// Pagination is optional: pass `()` for defaults or a bare `limit`.
    pub async fn list_sessions<P: Into<Pagination>>(&self, pagination: P) -> Result<SessionList> {
        let (limit, _offset) = pagination.into().resolve();
        let mut req = Request {
            method: "GET".into(),
            path: "/intelligence/sessions".into(),
            ..Default::default()
        };
        // The sessions endpoint paginates by `limit` only.
        push_pagination(&mut req.query, limit, 0);
        self.client.dispatcher().json(req).await
    }

    /// Fetch one intelligence session by id.
    pub async fn get_session(&self, session_id: &str) -> Result<IntelligenceSession> {
        self.client
            .dispatcher()
            .json(Request {
                method: "GET".into(),
                path: format!("/intelligence/sessions/{session_id}"),
                ..Default::default()
            })
            .await
    }

    /// Delete an intelligence session.
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.client
            .dispatcher()
            .empty(Request {
                method: "DELETE".into(),
                path: format!("/intelligence/sessions/{session_id}"),
                ..Default::default()
            })
            .await
    }

    /// Append a message to a session.
    pub async fn append_message<S: Into<String>, C: Into<String>>(
        &self,
        session_id: &str,
        role: S,
        content: C,
    ) -> Result<SessionMessage> {
        self.append_message_with(
            session_id,
            AppendMessageRequest {
                role: role.into(),
                content: content.into(),
                metadata: None,
            },
        )
        .await
    }

    /// Append a message to a session with explicit metadata.
    pub async fn append_message_with(
        &self,
        session_id: &str,
        request: AppendMessageRequest,
    ) -> Result<SessionMessage> {
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/intelligence/sessions/{session_id}/messages"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// List messages in a session.
    ///
    /// Pagination is optional: pass `()` for defaults or a bare `limit`.
    pub async fn list_messages<P: Into<Pagination>>(
        &self,
        session_id: &str,
        pagination: P,
    ) -> Result<MessageList> {
        let (limit, _offset) = pagination.into().resolve();
        let mut req = Request {
            method: "GET".into(),
            path: format!("/intelligence/sessions/{session_id}/messages"),
            ..Default::default()
        };
        push_pagination(&mut req.query, limit, 0);
        self.client.dispatcher().json(req).await
    }
}

/// One decoded server-sent event from the intelligence stream.
#[derive(Debug, Default, Clone)]
pub struct StreamEvent {
    /// The SSE `event:` name, when present.
    pub event: String,
    /// `chunk_type` field from the JSON body, when present.
    pub chunk_type: String,
    /// `content` field from the JSON body, when present.
    pub content: String,
    /// Free-form metadata attached to the chunk.
    pub metadata: Option<Metadata>,
    /// Raw JSON body bytes.
    pub raw: Vec<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamPayload {
    #[serde(default)]
    chunk_type: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    metadata: Option<Metadata>,
}

type ByteStream = Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + Unpin>;

/// Async iterator over [`StreamEvent`]s produced by an intelligence stream.
pub struct AskStream {
    inner: ByteStream,
    buffer: Vec<u8>,
    done: bool,
    pending_event: String,
    pending_data: Vec<String>,
}

impl AskStream {
    fn new(inner: ByteStream) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            done: false,
            pending_event: String::new(),
            pending_data: Vec::new(),
        }
    }

    /// Pull the next event from the stream. Returns `Ok(None)` at EOF.
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>> {
        loop {
            if let Some(event) = self.try_take_event() {
                return Ok(Some(event));
            }
            if self.done {
                if !self.pending_data.is_empty() {
                    let data = std::mem::take(&mut self.pending_data).join("\n");
                    let event = std::mem::take(&mut self.pending_event);
                    return Ok(Some(decode_event(event, data.into_bytes())));
                }
                return Ok(None);
            }
            match self.inner.next().await {
                Some(Ok(chunk)) => {
                    self.buffer.extend_from_slice(&chunk);
                }
                Some(Err(e)) => return Err(Error::Http(e)),
                None => {
                    self.done = true;
                    if !self.buffer.is_empty() {
                        // Flush any final pending lines without a trailing
                        // blank-line separator.
                        let line = std::mem::take(&mut self.buffer);
                        self.consume_line(&line);
                    }
                }
            }
        }
    }

    fn try_take_event(&mut self) -> Option<StreamEvent> {
        loop {
            let newline_pos = self.buffer.iter().position(|b| *b == b'\n')?;
            let line: Vec<u8> = self.buffer.drain(..=newline_pos).collect();
            // Drop trailing \r and \n.
            let mut end = line.len();
            if end > 0 && line[end - 1] == b'\n' {
                end -= 1;
            }
            if end > 0 && line[end - 1] == b'\r' {
                end -= 1;
            }
            let line = &line[..end];

            if line.is_empty() {
                if !self.pending_data.is_empty() {
                    let data = std::mem::take(&mut self.pending_data).join("\n");
                    let event = std::mem::take(&mut self.pending_event);
                    return Some(decode_event(event, data.into_bytes()));
                }
                self.pending_event.clear();
                continue;
            }
            self.consume_line(line);
        }
    }

    fn consume_line(&mut self, line: &[u8]) {
        if line.is_empty() {
            return;
        }
        if line[0] == b':' {
            // Comment line.
            return;
        }
        // Find optional ":" separator.
        if let Some(colon) = line.iter().position(|b| *b == b':') {
            let field = &line[..colon];
            let mut rest = &line[colon + 1..];
            if !rest.is_empty() && rest[0] == b' ' {
                rest = &rest[1..];
            }
            match field {
                b"event" => {
                    self.pending_event = String::from_utf8_lossy(rest).into_owned();
                }
                b"data" => {
                    self.pending_data
                        .push(String::from_utf8_lossy(rest).into_owned());
                }
                _ => {}
            }
        }
    }
}

fn decode_event(event: String, raw: Vec<u8>) -> StreamEvent {
    let payload: StreamPayload = serde_json::from_slice(&raw).unwrap_or_default();
    StreamEvent {
        event,
        chunk_type: payload.chunk_type,
        content: payload.content,
        metadata: payload.metadata,
        raw,
    }
}
