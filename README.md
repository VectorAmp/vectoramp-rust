# VectorAmp Rust SDK

[![pipeline status](https://gitlab.com/VectorAmp/SDK/Rust/badges/main/pipeline.svg)](https://gitlab.com/VectorAmp/SDK/Rust/-/commits/main)
[![coverage report](https://gitlab.com/VectorAmp/SDK/Rust/badges/main/coverage.svg)](https://gitlab.com/VectorAmp/SDK/Rust/-/commits/main)

Idiomatic async Rust client for the public VectorAmp API.

- Default API base URL: `https://api.vectoramp.com`
- Auth: `X-API-Key: <api_key>`
- Async / await on top of `reqwest` and `tokio`, with a small `Transport` trait so a different stack (gRPC, mocks) can be plugged in
- Dataset creation always uses SABLE; the SDK intentionally does not expose an index type option

> This crate is source-ready. It has not been published or tagged yet.

## Install

Pre-launch the crate is consumed directly from the GitLab repository:

```toml
# Cargo.toml
[dependencies]
vectoramp = { git = "https://gitlab.com/VectorAmp/SDK/Rust.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

After launch, the crate will be published to crates.io as `vectoramp`:

```toml
[dependencies]
vectoramp = "0.1"
```

## Quick start

```rust
use vectoramp::{Client, CreateDatasetRequest, EmbeddingConfig};

#[tokio::main]
async fn main() -> vectoramp::Result<()> {
    let client = Client::new(std::env::var("VECTORAMP_API_KEY").unwrap());

    let dataset = client
        .datasets()
        .create(CreateDatasetRequest {
            name: "product-docs".into(),
            dim: 2560,
            metric: Some("cosine".into()),
            embedding: Some(EmbeddingConfig {
                provider: Some("vectoramp".into()),
                model: Some("VectorAmp-Embedding-2560".into()),
            }),
            ..Default::default()
        })
        .await?;

    dataset
        .add_texts(vec!["VectorAmp is a high-performance vector database."])
        .await?;

    let answer = dataset.ask("What is VectorAmp?").await?;
    println!("{}", answer.answer);
    Ok(())
}
```

## Configure the client

```rust
use vectoramp::Client;

let client = Client::builder()
    .api_key(std::env::var("VECTORAMP_API_KEY").unwrap())
    .base_url("https://api.vectoramp.com")
    .build()?;
```

Custom HTTP client:

```rust
let http = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(60))
    .build()?;
let client = Client::builder()
    .api_key(api_key)
    .http_client(http)
    .build()?;
```

Custom transport for tests or future protocols:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use vectoramp::{Client, Request, Response, Transport};

struct MyTransport;

#[async_trait]
impl Transport for MyTransport {
    async fn send(&self, _req: Request) -> vectoramp::Result<Response> {
        unimplemented!()
    }
}

let client = Client::builder()
    .api_key(api_key)
    .transport(Arc::new(MyTransport))
    .build()?;
```

## Datasets

### List / get / create / delete

```rust
let page = client.datasets().list(50, 0).await?;

let dataset = client.datasets().get("dataset-id").await?;

let created = client
    .datasets()
    .create(CreateDatasetRequest {
        name: "docs".into(),
        dim: 2560,
        metric: Some("cosine".into()),
        ..Default::default()
    })
    .await?;

created.delete().await?;
```

`CreateDatasetRequest` does not include an `index_type` field. The SDK always sends `index_type: "sable"`.

`create`, `get`, and `list` return `Dataset` resource handles bound to the originating client. You can use either resource-style or service-style calls:

```rust
let dataset = client.datasets().get("dataset-id").await?;
let resp = dataset.search("hello").await?;

// Service-style remains supported.
let resp = client.datasets().search("dataset-id", "hello").await?;
```

### Source documents

Datasets can expose retained original source documents from ingestion or file upload. Document listing is cursor-based: pass `next_cursor` from the previous response and do not assume offsets or totals. `download_document` returns the original bytes and follows API/storage redirects.

```rust
use vectoramp::DocumentListOptions;

let page = dataset
    .list_documents(DocumentListOptions {
        limit: Some(50),
        status: Some("ready".into()),
        ..Default::default()
    })
    .await?;

for doc in &page.documents {
    if doc.download_available {
        let bytes = dataset.download_document(&doc.id).await?;
        let _ = bytes;
    }
}

if let Some(cursor) = page.next_cursor {
    let next = dataset
        .list_documents(DocumentListOptions {
            cursor: Some(cursor),
            ..Default::default()
        })
        .await?;
    let _ = next;
}
```

### Insert vectors

```rust
use vectoramp::Vector;

dataset
    .insert(vec![Vector {
        id: "doc-1".into(),
        values: vec![0.1, 0.2, 0.3],
        metadata: Some([("title".into(), serde_json::json!("Intro"))].into_iter().collect()),
    }])
    .await?;
```

### Add texts

`add_texts` embeds text through the dataset embedding model and inserts the resulting vectors. For quick inserts pass a `&str`, `Vec<&str>`, or `Vec<String>`; the SDK generates stable IDs (`text-1`, `text-2`, …). Pass a `Vec<TextDocument>` when you need custom IDs or metadata.

```rust
use vectoramp::TextDocument;

dataset.add_texts(vec!["Hello world", "Machine learning notes"]).await?;

dataset
    .add_texts(vec![
        TextDocument { id: "doc-1".into(), text: "Hello world".into(), metadata: None },
        TextDocument { id: "doc-2".into(), text: "Machine learning notes".into(), metadata: None },
    ])
    .await?;
```

### Search

```rust
use vectoramp::{SearchOptions, SearchInput};

let resp = dataset.search("machine learning best practices").await?;

let resp = dataset
    .search_with(
        "machine learning best practices",
        SearchOptions {
            top_k: Some(10),
            include_documents: Some(true),
            ..Default::default()
        },
    )
    .await?;

// Vector search:
let resp = dataset.search(SearchInput::Vector(vec![0.1, 0.2, 0.3])).await?;
```

String searches default to `top_k: 10` when you omit `SearchOptions::top_k`.

## Ingestion

### Sources and jobs

```rust
use vectoramp::StartIngestionRequest;

let sources = client.ingestion().list_sources(50, 0).await?;
let source = client.ingestion().get_source(&sources.sources[0].id.clone().unwrap_or_default()).await?;

let dataset = client.datasets().get("dataset-id").await?;
let job = dataset.ingest_source(source.identifier().unwrap()).await?;

// Equivalent service-style call.
let job = client
    .ingestion()
    .start_job(StartIngestionRequest {
        source_id: source.identifier().unwrap().into(),
        dataset_id: dataset.id().into(),
        pipeline_id: None,
    })
    .await?;

let jobs = client.ingestion().list_jobs(Some("dataset-id"), 50, 0).await?;
let job = client.ingestion().get_job(&job.job_id).await?;
```

### Typed source builders

Typed builders make source creation safer while still preserving `CreateSourceRequest` for fully manual calls. Supported public `source_type` values include `s3`, `web`, `gcs`, `gdrive`, `file_upload`, and `jira`; use `GenericSource` as an escape hatch.

```rust
use vectoramp::{S3Source, WebSource, GoogleDriveSource, FileUploadSource, GenericSource};

let web = client
    .sources()
    .create_source(WebSource {
        start_urls: vec!["https://docs.example.com".into()],
        max_depth: Some(2),
        ..Default::default()
    })
    .await?;

let s3 = client
    .sources()
    .create_source(S3Source {
        bucket: "my-bucket".into(),
        prefix: Some("docs/".into()),
        region: Some("us-east-1".into()),
        access_key_id: Some(std::env::var("AWS_ACCESS_KEY_ID").unwrap()),
        secret_access_key: Some(std::env::var("AWS_SECRET_ACCESS_KEY").unwrap()),
        ..Default::default()
    })
    .await?;

let gdrive = client
    .sources()
    .create_source(GoogleDriveSource {
        auth_mode: Some("service_account".into()),
        service_account_json: Some(std::env::var("GOOGLE_SERVICE_ACCOUNT_JSON").unwrap()),
        folder_ids: vec!["folder-id".into()],
        ..Default::default()
    })
    .await?;

let upload = client.sources().create_source(FileUploadSource::default()).await?;

let custom = client
    .sources()
    .create_source(GenericSource {
        source_type: "custom".into(),
        name: "custom-source".into(),
        config: [("type".into(), serde_json::json!("custom"))].into_iter().collect(),
        ..Default::default()
    })
    .await?;
```

`Dataset::ingest_new_source` accepts any builder, creates the source, and starts the ingestion job:

```rust
let job = dataset
    .ingest_new_source(WebSource {
        start_urls: vec!["https://example.com/releases".into()],
        ..Default::default()
    })
    .await?;
```

### Filesystem upload ingestion

For local files, the SDK creates a `file_upload` source, initializes presigned uploads, uploads file bytes, and completes the upload.

```rust
use std::path::PathBuf;
use vectoramp::IngestFilesOptions;

let job = dataset
    .ingest_files(vec![PathBuf::from("./docs/guide.pdf")], None)
    .await?;

let job = dataset
    .ingest_files(
        vec![PathBuf::from("./docs/guide.pdf")],
        Some(IngestFilesOptions {
            source_name: Some("product-docs-upload".into()),
            ..Default::default()
        }),
    )
    .await?;
```

## Intelligence / RAG

### Non-streaming

```rust
use vectoramp::AskOptions;

let answer = client.ask("What are the key product features?").await?;

let answer = client
    .ask_with(
        "What are the key product features?",
        AskOptions::default().with_all_datasets().with_top_k(5),
    )
    .await?;

let answer = dataset.ask("What are the key product features?").await?;
```

### Streaming SSE

```rust
let mut stream = client
    .intelligence()
    .stream(
        "Summarize the launch plan",
        AskOptions::default().with_dataset("dataset-id"),
    )
    .await?;

while let Some(event) = stream.next_event().await? {
    if event.chunk_type == "text" {
        print!("{}", event.content);
    }
}
```

## Errors

Non-2xx responses surface as `Error::Api(ApiError)`.

```rust
match client.datasets().get("missing").await {
    Ok(dataset) => { let _ = dataset; }
    Err(vectoramp::Error::Api(err)) => {
        eprintln!("api error {}: {}", err.status, err.message);
    }
    Err(err) => {
        eprintln!("transport error: {err}");
    }
}
```

## Development

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

GitLab CI runs the format check, clippy, and the full test suite.
