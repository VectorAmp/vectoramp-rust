//! Dataset service and the [`Dataset`] resource handle.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::client::Client;
use crate::errors::{Error, Result};
use crate::ingestion::IngestFilesOptions;
use crate::intelligence::AskOptions;
use crate::sources::IntoCreateSourceRequest;
use crate::transport::Request;
use crate::types::{
    infer_embedding_dim, AddTextsResponse, AskResponse, CreateDatasetRequest, DatasetDocumentList,
    DatasetInfo, DatasetList, DocumentListOpts, EmbedRequest, EmbedResponse, EmbeddingConfig,
    InsertVectorsRequest, InsertVectorsResponse, Job, Metadata, Rerank, RerankConfig,
    SearchRequest, SearchResponse, TextDocument, Vector, VectorId, DEFAULT_EMBEDDING_MODEL,
    DEFAULT_EMBEDDING_PROVIDER,
};

/// Default search top_k applied when one is not supplied.
pub const DEFAULT_SEARCH_TOP_K: u32 = 10;

/// Default page size used by list helpers when no limit is supplied.
pub const DEFAULT_PAGE_LIMIT: u32 = 50;

/// Pagination argument accepted by the `list*` helpers.
///
/// Implements `From` for `()` (defaults), `(u32, u32)` for explicit
/// `(limit, offset)`, and `u32` for just a limit, so callers can write
/// `list(())`, `list((50, 0))`, or `list(50)` interchangeably without ever
/// being forced to spell out a default page size.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pagination {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Pagination {
    /// Resolve the effective `(limit, offset)`, applying the default page
    /// limit when no limit was supplied.
    pub(crate) fn resolve(self) -> (u32, u32) {
        (
            self.limit.unwrap_or(DEFAULT_PAGE_LIMIT),
            self.offset.unwrap_or(0),
        )
    }
}

impl From<()> for Pagination {
    fn from(_: ()) -> Self {
        Pagination::default()
    }
}

impl From<u32> for Pagination {
    fn from(limit: u32) -> Self {
        Pagination {
            limit: Some(limit),
            offset: None,
        }
    }
}

impl From<(u32, u32)> for Pagination {
    fn from((limit, offset): (u32, u32)) -> Self {
        Pagination {
            limit: Some(limit),
            offset: Some(offset),
        }
    }
}

/// Service for managing datasets and dataset-scoped operations.
#[derive(Clone)]
pub struct DatasetService {
    client: Client,
}

impl DatasetService {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// List datasets.
    ///
    /// Pagination is optional: pass `()` for sensible defaults
    /// (limit [`DEFAULT_PAGE_LIMIT`], offset 0), a `(limit, offset)` tuple, or a
    /// bare `limit`.
    ///
    /// ```no_run
    /// # async fn run(client: vectoramp::Client) -> vectoramp::Result<()> {
    /// let all = client.datasets().list(()).await?;        // defaults
    /// let page = client.datasets().list((50, 0)).await?;  // explicit
    /// # let _ = (all, page);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list<P: Into<Pagination>>(&self, pagination: P) -> Result<DatasetList> {
        let (limit, offset) = pagination.into().resolve();
        let mut req = Request {
            method: "GET".into(),
            path: "/datasets".into(),
            ..Default::default()
        };
        push_pagination(&mut req.query, limit, offset);
        self.client.dispatcher().json(req).await
    }

    /// Fetch one dataset by ID and return a bound [`Dataset`] resource.
    pub async fn get(&self, dataset_id: &str) -> Result<Dataset> {
        let info: DatasetInfo = self
            .client
            .dispatcher()
            .json(Request {
                method: "GET".into(),
                path: format!("/datasets/{dataset_id}"),
                ..Default::default()
            })
            .await?;
        Ok(Dataset::new(self.client.clone(), info))
    }

    /// Create a SABLE dataset and return a bound [`Dataset`] resource.
    ///
    /// Accepts anything convertible into a [`CreateDatasetRequest`]: a bare name
    /// (`create("docs")`), a [`CreateDatasetBuilder`](crate::CreateDatasetBuilder)
    /// (`create(CreateDatasetRequest::builder("docs").hybrid(true))`), or a full
    /// [`CreateDatasetRequest`].
    ///
    /// With only a name, the SDK defaults the embedding to
    /// `VectorAmp-Embedding-4B` (provider `vectoramp`), infers `dim` (`2560`),
    /// and defaults the metric to `cosine`. Public dataset creation is
    /// SABLE-only; the SDK always sends `index_type: "sable"` and never exposes
    /// it. Supplying a custom/unknown embedding model without a `dim` is an
    /// error.
    ///
    /// ```no_run
    /// # use vectoramp::CreateDatasetRequest;
    /// # async fn run(client: vectoramp::Client) -> vectoramp::Result<()> {
    /// let ds = client.datasets().create("docs").await?;               // minimal
    /// let hy = client.datasets()
    ///     .create(CreateDatasetRequest::builder("docs").hybrid(true)) // hybrid
    ///     .await?;
    /// # let _ = (ds, hy);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create<R: Into<CreateDatasetRequest>>(&self, request: R) -> Result<Dataset> {
        let mut request = request.into();
        self.apply_create_defaults(&mut request)?;

        let mut body = serde_json::to_value(&request)?;
        if let Value::Object(ref mut map) = body {
            map.insert("index_type".into(), Value::String("sable".into()));
        }
        let info: DatasetInfo = self
            .client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/datasets".into(),
                body: Some(body),
                ..Default::default()
            })
            .await?;
        Ok(Dataset::new(self.client.clone(), info))
    }

    /// Fill in the SDK-side defaults for a create request: default embedding,
    /// inferred dimension, and default metric.
    fn apply_create_defaults(&self, request: &mut CreateDatasetRequest) -> Result<()> {
        // Default the embedding when one was not supplied.
        let embedding = request
            .embedding
            .get_or_insert_with(EmbeddingConfig::vectoramp);
        if embedding.provider.is_none() {
            embedding.provider = Some(DEFAULT_EMBEDDING_PROVIDER.into());
        }
        if embedding.model.is_none() {
            embedding.model = Some(DEFAULT_EMBEDDING_MODEL.into());
        }

        // Infer the dimension from the embedding model when not given.
        if request.dim.is_none() {
            let provider = request
                .embedding
                .as_ref()
                .and_then(|e| e.provider.as_deref());
            let model = request.embedding.as_ref().and_then(|e| e.model.as_deref());
            match infer_embedding_dim(provider, model) {
                Some(dim) => request.dim = Some(dim),
                None => {
                    return Err(Error::invalid_input(format!(
                        "could not infer dim for embedding model {:?}; pass dim explicitly via CreateDatasetRequest::builder(name).dim(n)",
                        model.unwrap_or(DEFAULT_EMBEDDING_MODEL)
                    )));
                }
            }
        }

        if request.metric.is_none() {
            request.metric = Some("cosine".into());
        }
        Ok(())
    }

    /// Delete a dataset by ID.
    pub async fn delete(&self, dataset_id: &str) -> Result<()> {
        self.client
            .dispatcher()
            .empty(Request {
                method: "DELETE".into(),
                path: format!("/datasets/{dataset_id}"),
                ..Default::default()
            })
            .await
    }

    /// List retained source documents using cursor pagination.
    pub async fn list_documents(
        &self,
        dataset_id: &str,
        opts: DocumentListOpts,
    ) -> Result<DatasetDocumentList> {
        let mut req = Request {
            method: "GET".into(),
            path: format!("/datasets/{dataset_id}/documents"),
            ..Default::default()
        };
        if let Some(limit) = opts.limit {
            if limit > 0 {
                req.query.push(("limit".into(), limit.to_string()));
            }
        }
        if let Some(cursor) = opts.cursor {
            if !cursor.is_empty() {
                req.query.push(("cursor".into(), cursor));
            }
        }
        if let Some(status) = opts.status {
            if !status.is_empty() {
                req.query.push(("status".into(), status));
            }
        }
        self.client.dispatcher().json(req).await
    }

    /// Download retained original document bytes.
    pub async fn download_document(&self, dataset_id: &str, document_id: &str) -> Result<Vec<u8>> {
        self.client
            .dispatcher()
            .bytes(Request {
                method: "GET".into(),
                path: format!("/datasets/{dataset_id}/documents/{document_id}/download"),
                ..Default::default()
            })
            .await
    }

    /// Search a dataset by text, vector, or full [`SearchRequest`].
    pub async fn search<I: Into<SearchInput>>(
        &self,
        dataset_id: &str,
        input: I,
    ) -> Result<SearchResponse> {
        self.search_with(dataset_id, input, SearchOptions::default())
            .await
    }

    /// Search with additional options.
    pub async fn search_with<I: Into<SearchInput>>(
        &self,
        dataset_id: &str,
        input: I,
        options: SearchOptions,
    ) -> Result<SearchResponse> {
        let mut req = match input.into() {
            SearchInput::Text(text) => SearchRequest {
                query_text: Some(text),
                top_k: 0,
                ..Default::default()
            },
            SearchInput::Vector(values) => SearchRequest {
                query: Some(values),
                top_k: 0,
                ..Default::default()
            },
            SearchInput::Request(r) => *r,
        };
        if let Some(top_k) = options.top_k {
            req.top_k = top_k;
        }
        if req.top_k == 0 {
            req.top_k = DEFAULT_SEARCH_TOP_K;
        }
        if let Some(im) = options.include_metadata {
            req.include_metadata = Some(im);
        }
        if let Some(id) = options.include_documents {
            req.include_documents = Some(id);
        }
        if let Some(filters) = options.filters {
            req.filters = Some(filters);
        }
        if let Some(rerank) = options.rerank {
            req.rerank = Some(rerank);
        }
        let body = serde_json::to_value(&req)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/datasets/{dataset_id}/search"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Insert vectors into a dataset.
    ///
    /// Vector ids accept strings or integers (see [`VectorId`]); integer ids are
    /// serialized as JSON numbers so the API preserves them.
    pub async fn insert(
        &self,
        dataset_id: &str,
        vectors: Vec<Vector>,
    ) -> Result<InsertVectorsResponse> {
        let body = serde_json::to_value(InsertVectorsRequest { vectors })?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/datasets/{dataset_id}/insert"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Alias for [`DatasetService::insert`] matching the cross-language
    /// `insertVectors` name.
    pub async fn insert_vectors(
        &self,
        dataset_id: &str,
        vectors: Vec<Vector>,
    ) -> Result<InsertVectorsResponse> {
        self.insert(dataset_id, vectors).await
    }

    /// Generate embeddings using the dataset embedding configuration.
    pub async fn embed(&self, dataset_id: &str, request: EmbedRequest) -> Result<EmbedResponse> {
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/datasets/{dataset_id}/embed"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Embed a list of texts and insert them as vectors. Generated IDs use the
    /// pattern `text-1`, `text-2`, … unless [`TextDocument::id`] is supplied.
    pub async fn add_texts<I: Into<AddTextsInput>>(
        &self,
        dataset_id: &str,
        input: I,
    ) -> Result<AddTextsResponse> {
        self.add_texts_with(dataset_id, input, AddTextsOptions::default())
            .await
    }

    /// Like [`DatasetService::add_texts`] but with explicit options.
    pub async fn add_texts_with<I: Into<AddTextsInput>>(
        &self,
        dataset_id: &str,
        input: I,
        options: AddTextsOptions,
    ) -> Result<AddTextsResponse> {
        let docs = match input.into() {
            AddTextsInput::Texts(texts) => texts
                .into_iter()
                .enumerate()
                .map(|(i, text)| TextDocument {
                    id: VectorId::Str(format!("text-{}", i + 1)),
                    text,
                    metadata: None,
                })
                .collect::<Vec<_>>(),
            AddTextsInput::Documents(docs) => docs,
        };

        if docs.is_empty() {
            return Ok(AddTextsResponse::default());
        }

        let texts: Vec<String> = docs.iter().map(|d| d.text.clone()).collect();
        let embed_req = EmbedRequest {
            texts: Some(texts),
            embedding_provider: options.embedding_provider.clone(),
            embedding_model: options.embedding_model.clone(),
            ..Default::default()
        };
        let embed_resp = self.embed(dataset_id, embed_req).await?;
        let mut embeddings = embed_resp.embeddings;
        if embeddings.is_empty() {
            if let Some(single) = embed_resp.embedding {
                embeddings.push(single);
            }
        }

        let vectors: Vec<Vector> = docs
            .into_iter()
            .enumerate()
            .map(|(i, doc)| {
                let mut metadata = doc.metadata.unwrap_or_default();
                metadata
                    .entry("text".to_string())
                    .or_insert(Value::String(doc.text));
                let values = embeddings.get(i).cloned().unwrap_or_default();
                Vector {
                    id: doc.id,
                    values,
                    metadata: Some(metadata),
                }
            })
            .collect();

        let inserted = self.insert(dataset_id, vectors).await?;
        Ok(AddTextsResponse {
            inserted: inserted.inserted,
            embeddings: embeddings.len() as u32,
        })
    }

    /// Upload local files into a dataset and return the ingestion job.
    pub async fn ingest_files(
        &self,
        dataset_id: &str,
        paths: Vec<PathBuf>,
        options: Option<IngestFilesOptions>,
    ) -> Result<Job> {
        self.client
            .ingestion()
            .ingest_files(dataset_id, paths, options)
            .await
    }

    /// Start ingestion from an existing source ID.
    pub async fn ingest_source(&self, dataset_id: &str, source_id: &str) -> Result<Job> {
        self.client
            .ingestion()
            .start_job(crate::types::StartIngestionRequest {
                source_id: source_id.into(),
                dataset_id: dataset_id.into(),
                pipeline_id: None,
            })
            .await
    }

    /// Start ingestion from an existing source ID with an explicit pipeline.
    pub async fn ingest_source_with_pipeline(
        &self,
        dataset_id: &str,
        source_id: &str,
        pipeline_id: &str,
    ) -> Result<Job> {
        self.client
            .ingestion()
            .start_job(crate::types::StartIngestionRequest {
                source_id: source_id.into(),
                dataset_id: dataset_id.into(),
                pipeline_id: Some(pipeline_id.into()),
            })
            .await
    }

    /// Create a new source from a typed builder and immediately start an
    /// ingestion job for this dataset.
    pub async fn ingest_new_source<B: IntoCreateSourceRequest>(
        &self,
        dataset_id: &str,
        builder: B,
    ) -> Result<Job> {
        let source = self.client.ingestion().create_source(builder).await?;
        let id = source
            .identifier()
            .ok_or_else(|| {
                Error::invalid_input("source response did not include id, source_id, or uuid")
            })?
            .to_owned();
        self.ingest_source(dataset_id, &id).await
    }

    /// Run an intelligence query scoped to one dataset.
    pub async fn ask<S: Into<String>>(&self, dataset_id: &str, query: S) -> Result<AskResponse> {
        self.ask_with(dataset_id, query, AskOptions::default())
            .await
    }

    /// Like [`DatasetService::ask`] but with explicit options.
    pub async fn ask_with<S: Into<String>>(
        &self,
        dataset_id: &str,
        query: S,
        mut options: AskOptions,
    ) -> Result<AskResponse> {
        options.dataset_id = Some(json!(dataset_id.to_owned()));
        self.client
            .intelligence()
            .ask_with(query.into(), options)
            .await
    }

    /// Open a streaming intelligence query scoped to one dataset.
    pub async fn ask_stream<S: Into<String>>(
        &self,
        dataset_id: &str,
        query: S,
    ) -> Result<crate::intelligence::AskStream> {
        let options = AskOptions::default().with_dataset(dataset_id.to_owned());
        self.client
            .intelligence()
            .stream(query.into(), options)
            .await
    }
}

/// Search input forms accepted by [`DatasetService::search`].
pub enum SearchInput {
    Text(String),
    Vector(Vec<f64>),
    Request(Box<SearchRequest>),
}

impl From<&str> for SearchInput {
    fn from(value: &str) -> Self {
        SearchInput::Text(value.to_owned())
    }
}

impl From<String> for SearchInput {
    fn from(value: String) -> Self {
        SearchInput::Text(value)
    }
}

impl From<Vec<f64>> for SearchInput {
    fn from(value: Vec<f64>) -> Self {
        SearchInput::Vector(value)
    }
}

impl From<SearchRequest> for SearchInput {
    fn from(value: SearchRequest) -> Self {
        SearchInput::Request(Box::new(value))
    }
}

/// Optional knobs passed to [`DatasetService::search_with`].
#[derive(Debug, Default, Clone)]
pub struct SearchOptions {
    pub top_k: Option<u32>,
    pub include_metadata: Option<bool>,
    pub include_documents: Option<bool>,
    pub filters: Option<HashMap<String, String>>,
    pub rerank: Option<Rerank>,
}

impl SearchOptions {
    /// Enable or disable VectorAmp reranking.
    pub fn with_rerank(mut self, enabled: bool) -> Self {
        self.rerank = Some(Rerank::Enabled(enabled));
        self
    }

    /// Set rerank options. Only enabled is required; defaults are vectoramp / VectorAmp-Rerank-v1.
    pub fn with_rerank_config(mut self, config: RerankConfig) -> Self {
        self.rerank = Some(Rerank::Config(config));
        self
    }
}

/// Input forms accepted by [`DatasetService::add_texts`].
pub enum AddTextsInput {
    Texts(Vec<String>),
    Documents(Vec<TextDocument>),
}

impl From<&str> for AddTextsInput {
    fn from(v: &str) -> Self {
        AddTextsInput::Texts(vec![v.to_owned()])
    }
}

impl From<String> for AddTextsInput {
    fn from(v: String) -> Self {
        AddTextsInput::Texts(vec![v])
    }
}

impl From<Vec<String>> for AddTextsInput {
    fn from(v: Vec<String>) -> Self {
        AddTextsInput::Texts(v)
    }
}

impl From<Vec<&str>> for AddTextsInput {
    fn from(v: Vec<&str>) -> Self {
        AddTextsInput::Texts(v.into_iter().map(|s| s.to_owned()).collect())
    }
}

impl From<Vec<TextDocument>> for AddTextsInput {
    fn from(v: Vec<TextDocument>) -> Self {
        AddTextsInput::Documents(v)
    }
}

/// Optional knobs passed to [`DatasetService::add_texts_with`].
#[derive(Debug, Default, Clone)]
pub struct AddTextsOptions {
    pub embedding_provider: Option<String>,
    pub embedding_model: Option<String>,
}

/// Bound dataset resource. Created by [`DatasetService::get`],
/// [`DatasetService::create`], or [`Client::dataset`].
#[derive(Clone)]
pub struct Dataset {
    client: Client,
    pub info: DatasetInfo,
}

impl Dataset {
    pub(crate) fn new(client: Client, info: DatasetInfo) -> Self {
        Self { client, info }
    }

    /// Underlying dataset id.
    pub fn id(&self) -> &str {
        &self.info.id
    }

    /// Search this dataset.
    pub async fn search<I: Into<SearchInput>>(&self, input: I) -> Result<SearchResponse> {
        self.client.datasets().search(self.id(), input).await
    }

    /// Search this dataset with explicit options.
    pub async fn search_with<I: Into<SearchInput>>(
        &self,
        input: I,
        options: SearchOptions,
    ) -> Result<SearchResponse> {
        self.client
            .datasets()
            .search_with(self.id(), input, options)
            .await
    }

    /// Insert vectors into this dataset.
    pub async fn insert(&self, vectors: Vec<Vector>) -> Result<InsertVectorsResponse> {
        self.client.datasets().insert(self.id(), vectors).await
    }

    /// Alias for [`Dataset::insert`] matching the cross-language `insertVectors`
    /// name.
    pub async fn insert_vectors(&self, vectors: Vec<Vector>) -> Result<InsertVectorsResponse> {
        self.client.datasets().insert(self.id(), vectors).await
    }

    /// Embed and insert texts into this dataset.
    pub async fn add_texts<I: Into<AddTextsInput>>(&self, input: I) -> Result<AddTextsResponse> {
        self.client.datasets().add_texts(self.id(), input).await
    }

    /// Like [`Dataset::add_texts`] but with explicit options.
    pub async fn add_texts_with<I: Into<AddTextsInput>>(
        &self,
        input: I,
        options: AddTextsOptions,
    ) -> Result<AddTextsResponse> {
        self.client
            .datasets()
            .add_texts_with(self.id(), input, options)
            .await
    }

    /// Generate embeddings via this dataset.
    pub async fn embed(&self, request: EmbedRequest) -> Result<EmbedResponse> {
        self.client.datasets().embed(self.id(), request).await
    }

    /// Run an intelligence query scoped to this dataset.
    pub async fn ask<S: Into<String>>(&self, query: S) -> Result<AskResponse> {
        self.client.datasets().ask(self.id(), query).await
    }

    /// Run an intelligence query with explicit options.
    pub async fn ask_with<S: Into<String>>(
        &self,
        query: S,
        options: AskOptions,
    ) -> Result<AskResponse> {
        self.client
            .datasets()
            .ask_with(self.id(), query, options)
            .await
    }

    /// Open a streaming intelligence query scoped to this dataset.
    pub async fn ask_stream<S: Into<String>>(
        &self,
        query: S,
    ) -> Result<crate::intelligence::AskStream> {
        self.client.datasets().ask_stream(self.id(), query).await
    }

    /// List retained source documents.
    pub async fn list_documents(&self, opts: DocumentListOpts) -> Result<DatasetDocumentList> {
        self.client.datasets().list_documents(self.id(), opts).await
    }

    /// Download retained original document bytes.
    pub async fn download_document(&self, document_id: &str) -> Result<Vec<u8>> {
        self.client
            .datasets()
            .download_document(self.id(), document_id)
            .await
    }

    /// Upload local files into this dataset.
    pub async fn ingest_files(
        &self,
        paths: Vec<PathBuf>,
        options: Option<IngestFilesOptions>,
    ) -> Result<Job> {
        self.client
            .datasets()
            .ingest_files(self.id(), paths, options)
            .await
    }

    /// Start ingestion from an existing source.
    pub async fn ingest_source(&self, source_id: &str) -> Result<Job> {
        self.client
            .datasets()
            .ingest_source(self.id(), source_id)
            .await
    }

    /// Start ingestion from an existing source with an explicit pipeline.
    pub async fn ingest_source_with_pipeline(
        &self,
        source_id: &str,
        pipeline_id: &str,
    ) -> Result<Job> {
        self.client
            .datasets()
            .ingest_source_with_pipeline(self.id(), source_id, pipeline_id)
            .await
    }

    /// Create a new source from a typed builder and start an ingestion job.
    pub async fn ingest_new_source<B: IntoCreateSourceRequest>(&self, builder: B) -> Result<Job> {
        self.client
            .datasets()
            .ingest_new_source(self.id(), builder)
            .await
    }

    /// Delete this dataset.
    pub async fn delete(self) -> Result<()> {
        self.client.datasets().delete(&self.info.id).await
    }
}

/// Convenience metadata builder for ad-hoc maps.
pub fn metadata() -> Metadata {
    Metadata::new()
}

pub(crate) fn push_pagination(query: &mut Vec<(String, String)>, limit: u32, offset: u32) {
    if limit > 0 {
        query.push(("limit".into(), limit.to_string()));
    }
    if offset > 0 {
        query.push(("offset".into(), offset.to_string()));
    }
}
