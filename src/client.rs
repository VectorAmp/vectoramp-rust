//! Top-level [`Client`] and [`ClientBuilder`].

use std::sync::Arc;

use url::Url;

use crate::datasets::DatasetService;
use crate::errors::Result;
use crate::ingestion::IngestionService;
use crate::intelligence::{AskOptions, AskStream, IntelligenceService};
use crate::schedules::ScheduleService;
use crate::transport::{Dispatcher, RestTransport, Transport};
use crate::types::AskResponse;
use crate::{DEFAULT_BASE_URL, USER_AGENT};

/// Root VectorAmp client.
///
/// Construct with [`Client::new`] for the simplest path, or
/// [`Client::builder`] for full configuration. The client is cheap to clone:
/// internal state is wrapped in an [`Arc`].
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) dispatcher: Dispatcher,
}

impl Client {
    /// Create a client targeting the production VectorAmp API with the given
    /// API key.
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        Self::builder()
            .api_key(api_key)
            .build()
            .expect("default client construction is infallible")
    }

    /// Start configuring a client with a builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Datasets service.
    pub fn datasets(&self) -> DatasetService {
        DatasetService::new(self.clone())
    }

    /// Ingestion service (sources, jobs, file uploads).
    pub fn ingestion(&self) -> IngestionService {
        IngestionService::new(self.clone())
    }

    /// Source-management alias for [`Client::ingestion`].
    pub fn sources(&self) -> IngestionService {
        self.ingestion()
    }

    /// Recurring ingestion schedule service.
    pub fn schedules(&self) -> ScheduleService {
        ScheduleService::new(self.clone())
    }

    /// Intelligence service (RAG queries).
    pub fn intelligence(&self) -> IntelligenceService {
        IntelligenceService::new(self.clone())
    }

    /// Run an intelligence query across the API default scope.
    ///
    /// Use [`Client::ask_with`] to attach options like `top_k` or a dataset
    /// scope.
    pub async fn ask<S: Into<String>>(&self, query: S) -> Result<AskResponse> {
        self.intelligence().ask(query.into()).await
    }

    /// Run an intelligence query with explicit options.
    pub async fn ask_with<S: Into<String>>(
        &self,
        query: S,
        options: AskOptions,
    ) -> Result<AskResponse> {
        self.intelligence().ask_with(query.into(), options).await
    }

    /// Open a streaming intelligence query across the API default scope.
    ///
    /// Use [`IntelligenceService::stream`] for explicit options.
    pub async fn ask_stream<S: Into<String>>(&self, query: S) -> Result<AskStream> {
        self.intelligence()
            .stream(query.into(), AskOptions::default())
            .await
    }

    pub(crate) fn dispatcher(&self) -> &Dispatcher {
        &self.inner.dispatcher
    }
}

/// Builder for [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    user_agent: Option<String>,
    http_client: Option<reqwest::Client>,
    transport: Option<Arc<dyn Transport>>,
}

impl ClientBuilder {
    /// Set the API key sent in the `X-API-Key` header.
    pub fn api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Override the API base URL. Defaults to [`DEFAULT_BASE_URL`].
    pub fn base_url<S: Into<String>>(mut self, base_url: S) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Override the User-Agent header sent with each request.
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Use a pre-configured [`reqwest::Client`] for the default transport.
    /// Ignored when [`ClientBuilder::transport`] is also set.
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Replace the entire transport. Useful for tests, gRPC, or custom HTTP
    /// stacks.
    pub fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Build the client. Returns an error only if a base URL is supplied that
    /// fails to parse.
    pub fn build(self) -> Result<Client> {
        let api_key = self.api_key.unwrap_or_default();
        let user_agent = self.user_agent.unwrap_or_else(|| USER_AGENT.to_owned());
        let transport: Arc<dyn Transport> = if let Some(t) = self.transport {
            t
        } else {
            let raw = self.base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
            let trimmed = raw.trim_end_matches('/').to_owned();
            let url = Url::parse(&trimmed)?;
            let mut rest = RestTransport::new(url, api_key, user_agent)?;
            if let Some(http) = self.http_client {
                rest = rest.with_http_client(http);
            }
            Arc::new(rest)
        };

        Ok(Client {
            inner: Arc::new(Inner {
                dispatcher: Dispatcher::new(transport),
            }),
        })
    }
}
