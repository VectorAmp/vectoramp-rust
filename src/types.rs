//! Core data types returned by and sent to the VectorAmp API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Free-form key/value metadata attached to resources and results.
pub type Metadata = HashMap<String, Value>;

/// Default embedding provider used when a dataset is created without one.
pub const DEFAULT_EMBEDDING_PROVIDER: &str = "vectoramp";
/// Default embedding model used when a dataset is created without one.
pub const DEFAULT_EMBEDDING_MODEL: &str = "VectorAmp-Embedding-4B";
/// Inferred dimensionality of [`DEFAULT_EMBEDDING_MODEL`].
pub const DEFAULT_EMBEDDING_DIM: u32 = 2560;

/// Embedding provider/model selector attached to a dataset.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl EmbeddingConfig {
    /// VectorAmp default embedding (`VectorAmp-Embedding-4B`, dim `2560`).
    pub fn vectoramp() -> Self {
        Self {
            provider: Some(DEFAULT_EMBEDDING_PROVIDER.into()),
            model: Some(DEFAULT_EMBEDDING_MODEL.into()),
        }
    }

    /// OpenAI embedding helper. Accepts the shorthands `"small"` /`"large"` or
    /// a full model id such as `text-embedding-3-small`.
    pub fn openai<S: Into<String>>(model: S) -> Self {
        let model = match model.into().as_str() {
            "small" => "text-embedding-3-small".to_string(),
            "large" => "text-embedding-3-large".to_string(),
            other => other.to_string(),
        };
        Self {
            provider: Some("openai".into()),
            model: Some(model),
        }
    }
}

/// Infer the embedding dimensionality from a `provider`/`model` pair using the
/// built-in table. Returns `None` for custom/unknown models, in which case the
/// caller must supply `dim` explicitly.
///
/// Known: `vectoramp/VectorAmp-Embedding-4B` → 2560,
/// `openai/text-embedding-3-small` → 1536, `openai/text-embedding-3-large` →
/// 3072.
pub fn infer_embedding_dim(provider: Option<&str>, model: Option<&str>) -> Option<u32> {
    match (provider, model) {
        (_, Some(DEFAULT_EMBEDDING_MODEL)) => Some(DEFAULT_EMBEDDING_DIM),
        (Some("openai"), Some("text-embedding-3-small")) => Some(1536),
        (Some("openai"), Some("text-embedding-3-large")) => Some(3072),
        // Allow inference even when the provider is omitted but the model is a
        // well-known OpenAI model.
        (_, Some("text-embedding-3-small")) => Some(1536),
        (_, Some("text-embedding-3-large")) => Some(3072),
        _ => None,
    }
}

/// VectorAmp dataset returned by the API.
///
/// Public dataset creation is SABLE-only, so newly created datasets always
/// come back with `index_type: "sable"`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    pub dim: u32,
    pub metric: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning: Option<HashMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// Paginated list of datasets.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DatasetList {
    #[serde(default)]
    pub datasets: Vec<DatasetInfo>,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

/// Request body for [`DatasetService::create`](crate::DatasetService::create).
///
/// Only `name` is required. When `dim` is omitted the SDK infers it from the
/// embedding model (defaulting to `VectorAmp-Embedding-4B` → `2560`). The SDK
/// always sends `index_type: "sable"` since public dataset creation is
/// SABLE-only and never exposes the index type.
///
/// Prefer the [`DatasetService::create`](crate::DatasetService::create) helper
/// (`create("name")`) or [`CreateDatasetRequest::builder`] over filling this
/// struct manually.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CreateDatasetRequest {
    pub name: String,
    /// Embedding dimensionality. Optional: inferred from the embedding model
    /// when omitted. Serialized as `dim` (never `dimension`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning: Option<HashMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingConfig>,
    /// Enable hybrid (dense + sparse) indexing. Maps to `hybrid: true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl CreateDatasetRequest {
    /// Start a builder for a dataset with the given name. Everything else is
    /// optional and defaulted server-side or by the SDK.
    pub fn builder<S: Into<String>>(name: S) -> CreateDatasetBuilder {
        CreateDatasetBuilder::new(name)
    }
}

/// Fluent builder for [`CreateDatasetRequest`].
///
/// Only the dataset name is required. The default embedding is
/// `VectorAmp-Embedding-4B` (provider `vectoramp`), dim is inferred (`2560`),
/// and metric defaults to `cosine`.
#[derive(Debug, Clone)]
pub struct CreateDatasetBuilder {
    request: CreateDatasetRequest,
}

impl CreateDatasetBuilder {
    /// Begin building a create request for `name`.
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            request: CreateDatasetRequest {
                name: name.into(),
                ..Default::default()
            },
        }
    }

    /// Set the embedding dimensionality explicitly. Required only for
    /// custom/unknown embedding models the SDK cannot infer.
    pub fn dim(mut self, dim: u32) -> Self {
        self.request.dim = Some(dim);
        self
    }

    /// Set the distance metric (defaults to `cosine`).
    pub fn metric<S: Into<String>>(mut self, metric: S) -> Self {
        self.request.metric = Some(metric.into());
        self
    }

    /// Set the embedding provider/model.
    pub fn embedding(mut self, embedding: EmbeddingConfig) -> Self {
        self.request.embedding = Some(embedding);
        self
    }

    /// Use an OpenAI embedding model. Accepts `"small"`/`"large"` (or the full
    /// model id). Dimensionality is inferred (`1536`/`3072`).
    pub fn openai<S: Into<String>>(mut self, model: S) -> Self {
        self.request.embedding = Some(EmbeddingConfig::openai(model));
        self
    }

    /// Enable hybrid (dense + sparse) indexing.
    pub fn hybrid(mut self, hybrid: bool) -> Self {
        self.request.hybrid = Some(hybrid);
        self
    }

    /// Attach arbitrary metadata to the dataset.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.request.metadata = Some(metadata);
        self
    }

    /// Finish building. The SDK fills in defaults and dim inference at create
    /// time.
    pub fn build(self) -> CreateDatasetRequest {
        self.request
    }
}

impl From<CreateDatasetBuilder> for CreateDatasetRequest {
    fn from(b: CreateDatasetBuilder) -> Self {
        b.build()
    }
}

impl From<&str> for CreateDatasetRequest {
    fn from(name: &str) -> Self {
        CreateDatasetRequest {
            name: name.to_owned(),
            ..Default::default()
        }
    }
}

impl From<String> for CreateDatasetRequest {
    fn from(name: String) -> Self {
        CreateDatasetRequest {
            name,
            ..Default::default()
        }
    }
}

/// Identifier for a vector record.
///
/// A vector id may be a string **or** an integer. Integer ids are serialized as
/// JSON numbers (not strings) so the API preserves their numeric type and does
/// not rewrite them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VectorId {
    /// Integer id, serialized as a JSON number.
    Int(i64),
    /// String id, serialized as a JSON string.
    Str(String),
}

impl Default for VectorId {
    fn default() -> Self {
        VectorId::Str(String::new())
    }
}

impl std::fmt::Display for VectorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorId::Int(n) => write!(f, "{n}"),
            VectorId::Str(s) => write!(f, "{s}"),
        }
    }
}

impl From<String> for VectorId {
    fn from(s: String) -> Self {
        VectorId::Str(s)
    }
}

impl From<&str> for VectorId {
    fn from(s: &str) -> Self {
        VectorId::Str(s.to_owned())
    }
}

macro_rules! vector_id_from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for VectorId {
            fn from(n: $t) -> Self {
                VectorId::Int(n as i64)
            }
        }
    )*};
}
vector_id_from_int!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// One vector record to insert into a dataset.
///
/// The `id` accepts a string or integer via [`VectorId`]; integer ids are
/// preserved as JSON numbers on the wire.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Vector {
    pub id: VectorId,
    pub values: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl Vector {
    /// Construct a vector from an id (string or integer) and values.
    pub fn new<I: Into<VectorId>>(id: I, values: Vec<f64>) -> Self {
        Self {
            id: id.into(),
            values,
            metadata: None,
        }
    }

    /// Attach metadata to the vector.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct InsertVectorsRequest {
    pub vectors: Vec<Vector>,
}

/// Number of vectors written by [`DatasetService::insert`](crate::DatasetService::insert).
#[derive(Debug, Default, Clone, Deserialize)]
pub struct InsertVectorsResponse {
    #[serde(default)]
    pub inserted: u32,
}

/// One text document to be embedded and inserted by `add_texts`.
///
/// `id` accepts a string or integer via [`VectorId`]. When omitted in the
/// convenience helpers the SDK generates `text-1`, `text-2`, … ids.
#[derive(Debug, Default, Clone)]
pub struct TextDocument {
    pub id: VectorId,
    pub text: String,
    pub metadata: Option<Metadata>,
}

/// Counts returned by `add_texts`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AddTextsResponse {
    #[serde(default)]
    pub inserted: u32,
    #[serde(default)]
    pub embeddings: u32,
}

/// Embedding endpoint request body.
#[derive(Debug, Default, Clone, Serialize)]
pub struct EmbedRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
}

/// Embedding endpoint response.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct EmbedResponse {
    #[serde(default)]
    pub embeddings: Vec<Vec<f64>>,
    #[serde(default)]
    pub embedding: Option<Vec<f64>>,
}

/// Dataset search request body.
///
/// Provide either [`SearchRequest::query`] for vector search or
/// [`SearchRequest::query_text`] for text search. `top_k` defaults to 10 when
/// the SDK's `search` convenience helpers are used.
/// VectorAmp rerank settings. Only `enabled` is required; provider defaults to
/// `vectoramp` and model defaults to `VectorAmp-Rerank-v1`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Rerank {
    Enabled(bool),
    Config(RerankConfig),
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct RerankConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SearchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_provider: Option<String>,
    pub top_k: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_filters: Option<Vec<AdvancedFilter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nprobe_override: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerank_depth_override: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_embeddings: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_documents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_metadata: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerank: Option<Rerank>,
}

/// Structured metadata filter expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AdvancedFilter {
    pub field: String,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<Value>>,
}

/// One ranked search result.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SearchResult {
    pub id: Value,
    #[serde(default)]
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_value: Option<String>,
}

/// Dataset search response.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub query_time_ms: Option<f64>,
}

/// Ingestion source returned by the API.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Source {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub config: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl Source {
    /// Return the most authoritative identifier set on the source.
    pub fn identifier(&self) -> Option<&str> {
        self.id
            .as_deref()
            .or(self.source_id.as_deref())
            .or(self.uuid.as_deref())
    }
}

/// Paginated list of ingestion sources.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SourceList {
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

/// Request body for creating an ingestion source. Prefer the typed builders in
/// [`crate::sources`] over filling this struct manually.
#[derive(Debug, Default, Clone, Serialize)]
pub struct CreateSourceRequest {
    pub source_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub config: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// Ingestion job returned by the API.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Job {
    #[serde(default)]
    pub job_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub documents_processed: Option<u64>,
    #[serde(default)]
    pub vectors_inserted: Option<u64>,
    #[serde(default)]
    pub processing_time_seconds: Option<f64>,
    #[serde(default)]
    pub pipeline_result: Option<Value>,
    #[serde(default)]
    pub error_details: Option<Value>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub progress_percentage: Option<f64>,
    #[serde(default)]
    pub current_step: Option<String>,
}

/// Paginated list of ingestion jobs.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct JobList {
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

/// Request body for [`IngestionService::start_job`](crate::IngestionService::start_job).
#[derive(Debug, Default, Clone, Serialize)]
pub struct StartIngestionRequest {
    pub source_id: String,
    pub dataset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
}

/// Local file descriptor used when initializing a presigned upload.
#[derive(Debug, Default, Clone, Serialize)]
pub struct UploadFile {
    pub name: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct InitUploadRequest {
    pub files: Vec<UploadFile>,
}

/// One presigned upload destination returned by the upload init endpoint.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct UploadTarget {
    pub file_id: String,
    #[serde(default)]
    pub file_name: Option<String>,
    pub upload_url: String,
}

/// Response from the upload init endpoint.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct InitUploadResponse {
    pub job_id: String,
    #[serde(default)]
    pub uploads: Vec<UploadTarget>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct CompleteUploadRequest {
    pub job_id: String,
    pub file_ids: Vec<String>,
}

/// Conversation turn for [`AskRequest::conversation_history`].
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

/// Intelligence query request body.
///
/// `dataset_id` is optional. Pass a dataset id, `"all"`, or leave it unset for
/// the API default. Use the `Client::ask` and `Dataset::ask` helpers when you
/// want a single-call API.
#[derive(Debug, Default, Clone, Serialize)]
pub struct AskRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<Vec<ConversationMessage>>,
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_sources: Option<bool>,
}

/// Non-streaming intelligence response.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AskResponse {
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub sources: Vec<SourceCitation>,
    #[serde(default)]
    pub chunks: Vec<RagChunk>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
}

/// Source cited by an intelligence answer.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SourceCitation {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub relevance: Option<f64>,
    #[serde(default)]
    pub pages: Vec<u32>,
    #[serde(default)]
    pub chunk_count: Option<u32>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub chunks: Vec<RagChunk>,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

/// Retrieved chunk used by an intelligence answer.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct RagChunk {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub page: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
}

/// Body for creating an intelligence session.
#[derive(Debug, Default, Clone, Serialize)]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl From<()> for CreateSessionRequest {
    fn from(_: ()) -> Self {
        CreateSessionRequest::default()
    }
}

impl From<&str> for CreateSessionRequest {
    fn from(title: &str) -> Self {
        CreateSessionRequest {
            title: Some(title.to_owned()),
            ..Default::default()
        }
    }
}

impl From<String> for CreateSessionRequest {
    fn from(title: String) -> Self {
        CreateSessionRequest {
            title: Some(title),
            ..Default::default()
        }
    }
}

/// An intelligence session: a durable container for a RAG conversation.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct IntelligenceSession {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Paginated list of intelligence sessions.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SessionList {
    #[serde(default)]
    pub sessions: Vec<IntelligenceSession>,
}

/// Body for appending a message to an intelligence session.
#[derive(Debug, Default, Clone, Serialize)]
pub struct AppendMessageRequest {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// One message stored in an intelligence session.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SessionMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Paginated list of session messages.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct MessageList {
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
}

/// Cursor pagination options for dataset document listing.
#[derive(Debug, Default, Clone)]
pub struct DocumentListOpts {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub status: Option<String>,
}

/// Source/original document metadata returned by the dataset document catalog.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DatasetDocument {
    pub id: String,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub version: Option<i32>,
    #[serde(default)]
    pub chunk_count: Option<i32>,
    #[serde(default)]
    pub embeddings_count: Option<i32>,
    #[serde(default)]
    pub download_available: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Recurring ingestion schedule returned by the API.
///
/// A schedule pairs a source with a target dataset and a cron expression. The
/// server's ingestion scheduler daemon polls for due schedules and creates jobs
/// as they fire.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Schedule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub pipeline_id: Option<String>,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Paginated list of schedules.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ScheduleList {
    #[serde(default)]
    pub schedules: Vec<Schedule>,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

/// Body for creating an ingestion schedule.
#[derive(Debug, Default, Clone, Serialize)]
pub struct CreateScheduleRequest {
    pub source_id: String,
    pub dataset_id: String,
    pub cron: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// Body for updating an ingestion schedule. Only `Some` fields are sent.
#[derive(Debug, Default, Clone, Serialize)]
pub struct UpdateScheduleRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// Response from an immediate schedule trigger.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct TriggerScheduleResponse {
    #[serde(default)]
    pub job_id: Option<String>,
}

/// Cursor-paginated dataset document list.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DatasetDocumentList {
    #[serde(default)]
    pub documents: Vec<DatasetDocument>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}
