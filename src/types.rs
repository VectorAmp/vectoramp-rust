//! Core data types returned by and sent to the VectorAmp API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Free-form key/value metadata attached to resources and results.
pub type Metadata = HashMap<String, Value>;

/// Embedding provider/model selector attached to a dataset.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
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
/// `name` and `dim` are required by the API. The SDK always sends
/// `index_type: "sable"` since public dataset creation is SABLE-only.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CreateDatasetRequest {
    pub name: String,
    pub dim: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning: Option<HashMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// One vector record to insert into a dataset.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Vector {
    pub id: String,
    pub values: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
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
#[derive(Debug, Default, Clone)]
pub struct TextDocument {
    pub id: String,
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
    pub file_name: String,
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
