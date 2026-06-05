//! Ingestion service: sources, jobs, and the high-level file-upload flow.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::client::Client;
use crate::datasets::push_pagination;
use crate::errors::{Error, Result};
use crate::sources::{sanitize, IntoCreateSourceRequest};
use crate::transport::Request;
use crate::types::{
    CompleteUploadRequest, InitUploadRequest, InitUploadResponse, Job, JobList, Metadata, Source,
    SourceList, StartIngestionRequest, UploadFile,
};

/// Optional knobs for [`IngestionService::ingest_files`] and
/// [`crate::Dataset::ingest_files`].
#[derive(Debug, Default, Clone)]
pub struct IngestFilesOptions {
    pub source_name: Option<String>,
    pub description: Option<String>,
    pub pipeline_id: Option<String>,
    pub metadata: Option<Metadata>,
}

/// Cursor pagination options for dataset document listing. Re-exported as
/// [`crate::DocumentListOptions`].
pub type DocumentListOptions = crate::types::DocumentListOpts;

/// Service for managing ingestion sources, upload flows, and ingestion jobs.
#[derive(Clone)]
pub struct IngestionService {
    client: Client,
}

impl IngestionService {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// List ingestion sources.
    pub async fn list_sources(&self, limit: u32, offset: u32) -> Result<SourceList> {
        let mut req = Request {
            method: "GET".into(),
            path: "/ingestion/sources".into(),
            ..Default::default()
        };
        push_pagination(&mut req.query, limit, offset);
        self.client.dispatcher().json(req).await
    }

    /// Get one ingestion source by ID.
    pub async fn get_source(&self, source_id: &str) -> Result<Source> {
        self.client
            .dispatcher()
            .json(Request {
                method: "GET".into(),
                path: format!("/ingestion/sources/{source_id}"),
                ..Default::default()
            })
            .await
    }

    /// Create an ingestion source from any [`IntoCreateSourceRequest`] builder
    /// (or a [`crate::CreateSourceRequest`] directly).
    pub async fn create_source<B: IntoCreateSourceRequest>(&self, builder: B) -> Result<Source> {
        let body = serde_json::to_value(builder.into_create_source_request())?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/ingestion/sources".into(),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Start an ingestion job.
    pub async fn start_job(&self, request: StartIngestionRequest) -> Result<Job> {
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/ingestion/jobs".into(),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// List ingestion jobs, optionally filtered by `dataset_id`.
    pub async fn list_jobs(
        &self,
        dataset_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<JobList> {
        let mut req = Request {
            method: "GET".into(),
            path: "/ingestion/jobs".into(),
            ..Default::default()
        };
        push_pagination(&mut req.query, limit, offset);
        if let Some(id) = dataset_id {
            if !id.is_empty() {
                req.query.push(("dataset_id".into(), id.into()));
            }
        }
        self.client.dispatcher().json(req).await
    }

    /// Fetch one ingestion job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Job> {
        self.client
            .dispatcher()
            .json(Request {
                method: "GET".into(),
                path: format!("/ingestion/jobs/{job_id}"),
                ..Default::default()
            })
            .await
    }

    /// Queue a fresh full-rerun job from an eligible failed or cancelled job.
    pub async fn retry_job(&self, job_id: &str) -> Result<Job> {
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/ingestion/jobs/{job_id}/retry"),
                ..Default::default()
            })
            .await
    }

    /// Initialize presigned uploads for a `file_upload` source.
    pub async fn init_upload(
        &self,
        source_id: &str,
        files: Vec<UploadFile>,
    ) -> Result<InitUploadResponse> {
        let body = serde_json::to_value(InitUploadRequest { files })?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/ingestion/sources/{source_id}/upload/init"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Complete a presigned file-upload job after all PUTs finish.
    pub async fn complete_upload(
        &self,
        source_id: &str,
        job_id: &str,
        file_ids: Vec<String>,
    ) -> Result<Job> {
        let body = serde_json::to_value(CompleteUploadRequest {
            job_id: job_id.into(),
            file_ids,
        })?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/ingestion/sources/{source_id}/upload/complete"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Upload local files into `dataset_id` and return the ingestion job.
    ///
    /// The SDK creates a `file_upload` source automatically (using
    /// `options.source_name` when provided), initializes presigned uploads,
    /// PUTs each file, and completes the job.
    pub async fn ingest_files(
        &self,
        dataset_id: &str,
        paths: Vec<PathBuf>,
        options: Option<IngestFilesOptions>,
    ) -> Result<Job> {
        let options = options.unwrap_or_default();
        let source_name = options
            .source_name
            .clone()
            .unwrap_or_else(|| default_upload_source_name(dataset_id));
        let mut metadata: Metadata = options.metadata.clone().unwrap_or_default();
        metadata
            .entry("dataset_id".to_string())
            .or_insert(serde_json::Value::String(dataset_id.into()));

        let builder = crate::sources::FileUploadSource {
            name: Some(source_name),
            description: options.description.clone(),
            metadata: Some(metadata),
            ..Default::default()
        };
        let source = self.create_source(builder).await?;
        let source_id = source
            .identifier()
            .ok_or_else(|| {
                Error::invalid_input("source response did not include id, source_id, or uuid")
            })?
            .to_owned();

        let mut upload_files = Vec::with_capacity(paths.len());
        for path in &paths {
            let metadata = tokio::fs::metadata(path).await?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::invalid_input(format!("invalid file name: {path:?}")))?
                .to_owned();
            let content_type = mime_guess::from_path(path)
                .first()
                .map(|m| m.essence_str().to_owned())
                .unwrap_or_else(|| "application/octet-stream".into());
            upload_files.push(UploadFile {
                name,
                size_bytes: metadata.len(),
                content_type: Some(content_type),
            });
        }
        let init = self.init_upload(&source_id, upload_files.clone()).await?;

        let mut file_ids = Vec::with_capacity(init.uploads.len());
        let http = reqwest::Client::new();
        for (i, target) in init.uploads.iter().enumerate() {
            if i >= paths.len() {
                break;
            }
            let path = &paths[i];
            let bytes = tokio::fs::read(path).await?;
            let mut req = http.put(&target.upload_url).body(bytes);
            if let Some(ct) = upload_files.get(i).and_then(|u| u.content_type.as_ref()) {
                req = req.header("Content-Type", ct);
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                return Err(Error::Other(format!(
                    "file upload failed for {} (status {})",
                    path.display(),
                    resp.status()
                )));
            }
            file_ids.push(target.file_id.clone());
        }

        let mut job = self.complete_upload(&source_id, &init.job_id, file_ids).await?;
        if job.job_id.is_empty() {
            job.job_id = init.job_id;
        }
        Ok(job)
    }
}

fn default_upload_source_name(dataset_id: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if dataset_id.is_empty() {
        format!("rust-sdk-file-upload-{now}")
    } else {
        format!("rust-sdk-file-upload-{}-{}", sanitize(dataset_id), now)
    }
}
