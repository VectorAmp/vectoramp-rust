//! Typed builders for ingestion sources.
//!
//! Each builder fills in the required `source_type`, sensible config defaults,
//! and a default name when one is not supplied. The builders all implement
//! [`IntoCreateSourceRequest`] so they can be passed wherever a source
//! definition is expected.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::types::{CreateSourceRequest, Metadata};

/// Source-type identifier values used by the public API.
pub mod source_type {
    pub const S3: &str = "s3";
    pub const WEB: &str = "web";
    pub const GCS: &str = "gcs";
    pub const GDRIVE: &str = "gdrive";
    pub const JIRA: &str = "jira";
    pub const CONFLUENCE: &str = "confluence";
    pub const FILE_UPLOAD: &str = "file_upload";
}

/// Trait implemented by typed source builders so they can be converted into
/// [`CreateSourceRequest`].
pub trait IntoCreateSourceRequest {
    fn into_create_source_request(self) -> CreateSourceRequest;
}

impl IntoCreateSourceRequest for CreateSourceRequest {
    fn into_create_source_request(self) -> CreateSourceRequest {
        self
    }
}

/// Escape hatch for source types or configuration shapes not yet modeled by
/// this SDK.
#[derive(Debug, Default, Clone)]
pub struct GenericSource {
    pub source_type: String,
    pub name: String,
    pub description: Option<String>,
    pub config: HashMap<String, Value>,
    pub metadata: Option<Metadata>,
}

impl IntoCreateSourceRequest for GenericSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        CreateSourceRequest {
            source_type: self.source_type,
            name: self.name,
            description: self.description,
            config: self.config,
            metadata: self.metadata,
        }
    }
}

/// CSS selectors used by [`WebSource`] to pick out content from crawled pages.
#[derive(Debug, Default, Clone)]
pub struct WebSelectors {
    pub content: Option<String>,
    pub title: Option<String>,
    pub exclude: Vec<String>,
}

impl WebSelectors {
    fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(c) = &self.content {
            map.insert("content".into(), Value::String(c.clone()));
        }
        if let Some(t) = &self.title {
            map.insert("title".into(), Value::String(t.clone()));
        }
        if !self.exclude.is_empty() {
            map.insert(
                "exclude".into(),
                Value::Array(self.exclude.iter().cloned().map(Value::String).collect()),
            );
        }
        Value::Object(map)
    }
}

/// Web crawler ingestion source.
///
/// Name defaults to `web-<host>` from the first start URL, or
/// `rust-sdk-web-source` when no URL is supplied.
#[derive(Debug, Default, Clone)]
pub struct WebSource {
    pub name: Option<String>,
    pub start_urls: Vec<String>,
    pub max_depth: Option<u32>,
    pub max_pages: Option<u32>,
    pub allowed_domains: Vec<String>,
    pub rate_limit_ms: Option<u32>,
    pub respect_robots_txt: Option<bool>,
    pub include_assets: Option<bool>,
    pub max_assets_per_page: Option<u32>,
    pub selectors: Option<WebSelectors>,
    pub headers: HashMap<String, String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for WebSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::WEB.into()));
        config.insert("start_urls".into(), json!(self.start_urls));
        if let Some(v) = self.max_depth {
            config.insert("max_depth".into(), json!(v));
        }
        if let Some(v) = self.max_pages {
            config.insert("max_pages".into(), json!(v));
        }
        if !self.allowed_domains.is_empty() {
            config.insert("allowed_domains".into(), json!(self.allowed_domains));
        }
        if let Some(v) = self.rate_limit_ms {
            config.insert("rate_limit_ms".into(), json!(v));
        }
        if let Some(v) = self.respect_robots_txt {
            config.insert("respect_robots_txt".into(), json!(v));
        }
        if let Some(v) = self.include_assets {
            config.insert("include_assets".into(), json!(v));
        }
        if let Some(v) = self.max_assets_per_page {
            config.insert("max_assets_per_page".into(), json!(v));
        }
        if let Some(s) = &self.selectors {
            config.insert("selectors".into(), s.to_value());
        }
        if !self.headers.is_empty() {
            let mut h = serde_json::Map::new();
            for (k, v) in &self.headers {
                h.insert(k.clone(), Value::String(v.clone()));
            }
            config.insert("headers".into(), Value::Object(h));
        }
        merge_extra(&mut config, self.config_extra);

        let name = self
            .name
            .unwrap_or_else(|| web_default_name(&self.start_urls));

        CreateSourceRequest {
            source_type: source_type::WEB.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// Amazon S3 ingestion source.
#[derive(Debug, Default, Clone)]
pub struct S3Source {
    pub name: Option<String>,
    pub bucket: String,
    pub prefix: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub file_patterns: Vec<String>,
    pub max_file_size_mb: Option<u32>,
    pub sync_mode: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for S3Source {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::S3.into()));
        config.insert("bucket".into(), Value::String(self.bucket.clone()));
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "incremental".into())),
        );
        if let Some(v) = self.prefix {
            config.insert("prefix".into(), Value::String(v));
        }
        if let Some(v) = self.region {
            config.insert("region".into(), Value::String(v));
        }
        if let Some(v) = self.access_key_id {
            config.insert("access_key_id".into(), Value::String(v));
        }
        if let Some(v) = self.secret_access_key {
            config.insert("secret_access_key".into(), Value::String(v));
        }
        if !self.file_patterns.is_empty() {
            config.insert("file_patterns".into(), json!(self.file_patterns));
        }
        if let Some(v) = self.max_file_size_mb {
            config.insert("max_file_size_mb".into(), json!(v));
        }
        merge_extra(&mut config, self.config_extra);

        let name = self.name.unwrap_or_else(|| {
            if self.bucket.is_empty() {
                "rust-sdk-s3-source".into()
            } else {
                format!("s3-{}", sanitize(&self.bucket))
            }
        });

        CreateSourceRequest {
            source_type: source_type::S3.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// Google Cloud Storage ingestion source.
#[derive(Debug, Default, Clone)]
pub struct GcsSource {
    pub name: Option<String>,
    pub bucket: String,
    pub prefix: Option<String>,
    pub project_id: Option<String>,
    pub credentials_json: Option<String>,
    pub file_patterns: Vec<String>,
    pub max_file_size_mb: Option<u32>,
    pub sync_mode: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for GcsSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::GCS.into()));
        config.insert("bucket".into(), Value::String(self.bucket.clone()));
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "incremental".into())),
        );
        if let Some(v) = self.prefix {
            config.insert("prefix".into(), Value::String(v));
        }
        if let Some(v) = self.project_id {
            config.insert("project_id".into(), Value::String(v));
        }
        if let Some(v) = self.credentials_json {
            config.insert("credentials_json".into(), Value::String(v));
        }
        if !self.file_patterns.is_empty() {
            config.insert("file_patterns".into(), json!(self.file_patterns));
        }
        if let Some(v) = self.max_file_size_mb {
            config.insert("max_file_size_mb".into(), json!(v));
        }
        merge_extra(&mut config, self.config_extra);

        let name = self.name.unwrap_or_else(|| {
            if self.bucket.is_empty() {
                "rust-sdk-gcs-source".into()
            } else {
                format!("gcs-{}", sanitize(&self.bucket))
            }
        });

        CreateSourceRequest {
            source_type: source_type::GCS.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// Google Drive ingestion source. The public `source_type` is `gdrive`.
#[derive(Debug, Default, Clone)]
pub struct GoogleDriveSource {
    pub name: Option<String>,
    pub auth_mode: Option<String>,
    pub service_account_json: Option<String>,
    pub delegated_user: Option<String>,
    pub oauth_credentials: Option<HashMap<String, Value>>,
    pub drive_id: Option<String>,
    pub folder_ids: Vec<String>,
    pub query: Option<String>,
    pub mime_types: Vec<String>,
    pub include_shared_drives: Option<bool>,
    pub include_team_drives: Option<bool>,
    pub sync_mode: Option<String>,
    pub page_size: Option<u32>,
    pub resume_cursor: Option<String>,
    pub fetch_attachments: Option<bool>,
    pub sampling_enabled: Option<bool>,
    pub sampling_limit: Option<u32>,
    pub max_concurrent_downloads: Option<u32>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for GoogleDriveSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::GDRIVE.into()));
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "incremental".into())),
        );
        if let Some(v) = self.auth_mode {
            config.insert("auth_mode".into(), Value::String(v));
        }
        if let Some(v) = self.service_account_json {
            config.insert("service_account_json".into(), Value::String(v));
        }
        if let Some(v) = self.delegated_user {
            config.insert("delegated_user".into(), Value::String(v));
        }
        if let Some(v) = self.oauth_credentials {
            config.insert("oauth_credentials".into(), json!(v));
        }
        if let Some(v) = &self.drive_id {
            config.insert("drive_id".into(), Value::String(v.clone()));
        }
        if !self.folder_ids.is_empty() {
            config.insert("folder_ids".into(), json!(self.folder_ids));
        }
        if let Some(v) = self.query {
            config.insert("query".into(), Value::String(v));
        }
        if !self.mime_types.is_empty() {
            config.insert("mime_types".into(), json!(self.mime_types));
        }
        if let Some(v) = self.include_shared_drives {
            config.insert("include_shared_drives".into(), json!(v));
        }
        if let Some(v) = self.include_team_drives {
            config.insert("include_team_drives".into(), json!(v));
        }
        if let Some(v) = self.page_size {
            config.insert("page_size".into(), json!(v));
        }
        if let Some(v) = self.resume_cursor {
            config.insert("resume_cursor".into(), Value::String(v));
        }
        if let Some(v) = self.fetch_attachments {
            config.insert("fetch_attachments".into(), json!(v));
        }
        if let Some(v) = self.sampling_enabled {
            config.insert("sampling_enabled".into(), json!(v));
        }
        if let Some(v) = self.sampling_limit {
            config.insert("sampling_limit".into(), json!(v));
        }
        if let Some(v) = self.max_concurrent_downloads {
            config.insert("max_concurrent_downloads".into(), json!(v));
        }
        merge_extra(&mut config, self.config_extra);

        let name = self.name.clone().unwrap_or_else(|| {
            if let Some(d) = &self.drive_id {
                format!("gdrive-{}", sanitize(d))
            } else if let Some(f) = self.folder_ids.first() {
                if !f.is_empty() {
                    format!("gdrive-{}", sanitize(f))
                } else {
                    "rust-sdk-gdrive-source".into()
                }
            } else {
                "rust-sdk-gdrive-source".into()
            }
        });

        CreateSourceRequest {
            source_type: source_type::GDRIVE.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// File upload source used by the presigned upload flow. Prefer
/// [`crate::Dataset::ingest_files`] for the full local-file upload pipeline.
#[derive(Debug, Default, Clone)]
pub struct FileUploadSource {
    pub name: Option<String>,
    pub storage_provider: Option<String>,
    pub key_prefix_template: Option<String>,
    pub allowed_content_types: Vec<String>,
    pub max_file_size_mb: Option<u32>,
    pub max_files_per_job: Option<u32>,
    pub sync_mode: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for FileUploadSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert(
            "type".into(),
            Value::String(source_type::FILE_UPLOAD.into()),
        );
        config.insert(
            "storage_provider".into(),
            Value::String(self.storage_provider.unwrap_or_else(|| "s3".into())),
        );
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "full".into())),
        );
        if let Some(v) = self.key_prefix_template {
            config.insert("key_prefix_template".into(), Value::String(v));
        }
        if !self.allowed_content_types.is_empty() {
            config.insert(
                "allowed_content_types".into(),
                json!(self.allowed_content_types),
            );
        }
        if let Some(v) = self.max_file_size_mb {
            config.insert("max_file_size_mb".into(), json!(v));
        }
        if let Some(v) = self.max_files_per_job {
            config.insert("max_files_per_job".into(), json!(v));
        }
        merge_extra(&mut config, self.config_extra);

        let name = self.name.unwrap_or_else(|| "rust-sdk-file-upload".into());

        CreateSourceRequest {
            source_type: source_type::FILE_UPLOAD.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// Jira ingestion source.
#[derive(Debug, Default, Clone)]
pub struct JiraSource {
    pub name: Option<String>,
    pub cloud_id: String,
    pub access_token: Option<String>,
    pub project_keys: Vec<String>,
    pub jql: Option<String>,
    pub include_comments: Option<bool>,
    pub sync_mode: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for JiraSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::JIRA.into()));
        config.insert("cloud_id".into(), Value::String(self.cloud_id.clone()));
        config.insert(
            "include_comments".into(),
            Value::Bool(self.include_comments.unwrap_or(true)),
        );
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "incremental".into())),
        );
        if let Some(v) = self.access_token {
            config.insert("access_token".into(), Value::String(v));
        }
        if !self.project_keys.is_empty() {
            config.insert("project_keys".into(), json!(self.project_keys));
        }
        if let Some(v) = self.jql {
            config.insert("jql".into(), Value::String(v));
        }
        merge_extra(&mut config, self.config_extra);

        let hint = self
            .project_keys
            .first()
            .filter(|p| !p.is_empty())
            .cloned()
            .unwrap_or_else(|| self.cloud_id.clone());
        let name = self.name.unwrap_or_else(|| {
            if hint.is_empty() {
                "rust-sdk-jira-source".into()
            } else {
                format!("jira-{}", sanitize(&hint))
            }
        });

        CreateSourceRequest {
            source_type: source_type::JIRA.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

/// Confluence ingestion source (spaces and pages).
///
/// Provide either `cloud_id` (Atlassian OAuth site id) or `base_url`
/// (e.g. `https://company.atlassian.net`). `auth_mode` defaults to `basic`
/// (`username` + `api_token`); pass `oauth_credentials` for OAuth.
#[derive(Debug, Default, Clone)]
pub struct ConfluenceSource {
    pub name: Option<String>,
    pub cloud_id: Option<String>,
    pub base_url: Option<String>,
    pub auth_mode: Option<String>,
    pub username: Option<String>,
    pub api_token: Option<String>,
    pub oauth_credentials: Option<HashMap<String, Value>>,
    pub spaces: Vec<String>,
    pub include_attachments: Option<bool>,
    pub sync_mode: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub config_extra: HashMap<String, Value>,
}

impl IntoCreateSourceRequest for ConfluenceSource {
    fn into_create_source_request(self) -> CreateSourceRequest {
        let mut config: HashMap<String, Value> = HashMap::new();
        config.insert("type".into(), Value::String(source_type::CONFLUENCE.into()));
        config.insert(
            "auth_mode".into(),
            Value::String(self.auth_mode.unwrap_or_else(|| "basic".into())),
        );
        config.insert(
            "include_attachments".into(),
            Value::Bool(self.include_attachments.unwrap_or(false)),
        );
        config.insert(
            "sync_mode".into(),
            Value::String(self.sync_mode.unwrap_or_else(|| "incremental".into())),
        );
        if let Some(v) = &self.cloud_id {
            config.insert("cloud_id".into(), Value::String(v.clone()));
        }
        if let Some(v) = &self.base_url {
            config.insert("base_url".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.username {
            config.insert("username".into(), Value::String(v));
        }
        if let Some(v) = self.api_token {
            config.insert("api_token".into(), Value::String(v));
        }
        if let Some(v) = self.oauth_credentials {
            config.insert("oauth_credentials".into(), json!(v));
        }
        if !self.spaces.is_empty() {
            config.insert("spaces".into(), json!(self.spaces));
        }
        merge_extra(&mut config, self.config_extra);

        let hint = self
            .spaces
            .first()
            .filter(|s| !s.is_empty())
            .cloned()
            .or_else(|| confluence_host(self.base_url.as_deref()))
            .or_else(|| self.cloud_id.clone());
        let name = self.name.unwrap_or_else(|| match hint {
            Some(h) if !h.is_empty() => format!("confluence-{}", sanitize(&h)),
            _ => "rust-sdk-confluence-source".into(),
        });

        CreateSourceRequest {
            source_type: source_type::CONFLUENCE.into(),
            name,
            description: self.description,
            config,
            metadata: self.metadata,
        }
    }
}

fn confluence_host(base_url: Option<&str>) -> Option<String> {
    let url = base_url?;
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            return Some(host.to_owned());
        }
    }
    None
}

fn merge_extra(config: &mut HashMap<String, Value>, extra: HashMap<String, Value>) {
    for (k, v) in extra {
        config.insert(k, v);
    }
}

fn web_default_name(start_urls: &[String]) -> String {
    let first = match start_urls.iter().find(|s| !s.is_empty()) {
        Some(s) => s,
        None => return "rust-sdk-web-source".into(),
    };
    if let Ok(parsed) = url::Url::parse(first) {
        if let Some(host) = parsed.host_str() {
            return format!("web-{}", sanitize(host));
        }
    }
    format!("web-{}", sanitize(first))
}

/// Lowercase, dash-separated sanitization used to derive default source names.
pub(crate) fn sanitize(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut last_dash = false;
    for c in lower.chars() {
        let keep = c.is_ascii_lowercase() || c.is_ascii_digit();
        if keep {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "source".into()
    } else {
        trimmed
    }
}
