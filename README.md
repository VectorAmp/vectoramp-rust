<div align="center">
  <a href="https://vectoramp.com/">
    <picture>
      <source media="(prefers-color-scheme: light)" srcset="https://vectoramp.com/logo-full-light.svg">
      <source media="(prefers-color-scheme: dark)" srcset="https://vectoramp.com/logo-full-dark.svg">
      <img alt="VectorAmp Logo" src="https://vectoramp.com/logo-full-dark.svg" width="50%">
    </picture>
  </a>
</div>

# VectorAmp Rust SDK

Idiomatic async Rust client for the public VectorAmp API.

- Default API base URL: `https://api.vectoramp.com`
- Auth: `X-API-Key: <api_key>`
- Async / await on top of `reqwest` and `tokio`, with a small `Transport` trait so a different stack (gRPC, mocks) can be plugged in
- Dataset creation always uses SABLE; the SDK intentionally does not expose an index type option

Licensed under the [Apache License 2.0](LICENSE).

## Install

```toml
# Cargo.toml
[dependencies]
vectoramp = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

Only a name is required to create a dataset. The SDK defaults the embedding to
`VectorAmp-Embedding-4B` (provider `vectoramp`), infers the dimension (`2560`),
defaults the metric to `cosine`, and always uses the SABLE index.

```rust
use vectoramp::Client;

#[tokio::main]
async fn main() -> vectoramp::Result<()> {
    let client = Client::new(std::env::var("VECTORAMP_API_KEY").unwrap());

    let dataset = client.datasets().create("product-docs").await?;

    dataset
        .add_texts(vec!["VectorAmp is a high-performance vector database."])
        .await?;

    let answer = dataset.ask("What is VectorAmp?").await?;
    println!("{}", answer.answer);
    Ok(())
}
```

`Client::new` reads the API key you pass in; the only required input is the key,
and it is commonly read from `VECTORAMP_API_KEY`.

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

### Create

```rust
use vectoramp::CreateDatasetRequest;

// Minimal: name only (embedding + dim + metric defaulted/inferred).
let dataset = client.datasets().create("docs").await?;

// Hybrid (dense + sparse) index.
let hybrid = client
    .datasets()
    .create(CreateDatasetRequest::builder("docs").hybrid(true))
    .await?;

// OpenAI embedding ("small" → 1536, "large" → 3072 inferred).
let openai = client
    .datasets()
    .create(CreateDatasetRequest::builder("docs").openai("small"))
    .await?;

// Custom / unknown model requires an explicit dim.
let custom = client
    .datasets()
    .create(
        CreateDatasetRequest::builder("docs")
            .embedding(vectoramp::EmbeddingConfig {
                provider: Some("acme".into()),
                model: Some("acme-embed".into()),
            })
            .dim(1024),
    )
    .await?;
```

`CreateDatasetRequest` has no `index_type` field. The SDK always sends
`index_type: "sable"`. The create body field is `dim` (never `dimension`).

### List / get / delete

```rust
// Pagination is optional: pass `()` for defaults, `(limit, offset)`, or a bare limit.
let page = client.datasets().list(()).await?;
let page = client.datasets().list((50, 0)).await?;

let dataset = client.datasets().get("dataset-id").await?;
dataset.delete().await?;
```

`create`, `get`, and `list` return `Dataset` resource handles bound to the
originating client. Both the object→method and service styles work:

```rust
let dataset = client.datasets().get("dataset-id").await?;
let resp = dataset.search("hello").await?;          // object → method (preferred)

let resp = client.datasets().search("dataset-id", "hello").await?; // service style
```

### Insert vectors

Vector ids accept a **string or an integer**. Integer ids are serialized as JSON
numbers so the API preserves them exactly.

```rust
use vectoramp::Vector;

dataset
    .insert(vec![
        Vector::new(1, vec![0.1, 0.2, 0.3]),       // numeric id → JSON number
        Vector::new("doc-2", vec![0.4, 0.5, 0.6]), // string id  → JSON string
    ])
    .await?;

// `insert_vectors` is an alias of `insert`.
dataset.insert_vectors(vec![Vector::new(2, vec![0.7, 0.8, 0.9])]).await?;
```

### Add texts

`add_texts` embeds text through the dataset embedding model, copies the source
text into `metadata.text`, and inserts the resulting vectors. Pass a `&str`,
`Vec<&str>`, or `Vec<String>` (the SDK generates `text-1`, `text-2`, … ids), or
a `Vec<TextDocument>` for custom ids/metadata.

```rust
use vectoramp::TextDocument;

dataset.add_texts(vec!["Hello world", "Machine learning notes"]).await?;

dataset
    .add_texts(vec![
        TextDocument { id: "doc-1".into(), text: "Hello world".into(), metadata: None },
        TextDocument { id: 2.into(), text: "Numeric id".into(), metadata: None },
    ])
    .await?;
```

### Search

String queries default to `top_k: 10`. `rerank: true` expands to the full rerank
object. Hybrid search accepts `sparse_query`/`alpha` via `SearchRequest`.

```rust
use vectoramp::{SearchInput, SearchOptions};

let resp = dataset.search("machine learning best practices").await?;

let resp = dataset
    .search_with(
        "machine learning best practices",
        SearchOptions {
            top_k: Some(10),
            include_documents: Some(true),
            ..Default::default()
        }
        .with_rerank(true), // expands to vectoramp / VectorAmp-Rerank-v1
    )
    .await?;

// Vector search:
let resp = dataset.search(SearchInput::Vector(vec![0.1, 0.2, 0.3])).await?;
```

### Source documents

Document listing is cursor-based: pass `next_cursor` from the previous response.
`download_document` returns the original bytes and follows redirects.

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
        let _bytes = dataset.download_document(&doc.id).await?;
    }
}
```

## Ingestion

### Source builders

Typed builders fill in `source_type`, sensible defaults, and a generated name.
Supported types: `web`, `s3`, `gcs`, `gdrive`, `jira`, `confluence`,
`file_upload`, plus `GenericSource` as an escape hatch.

```rust
use vectoramp::{ConfluenceSource, S3Source, WebSource};

let web = client
    .sources()
    .create_web(WebSource {
        start_urls: vec!["https://docs.example.com".into()],
        max_depth: Some(2),
        ..Default::default()
    })
    .await?;

let s3 = client
    .sources()
    .create_s3(S3Source {
        bucket: "my-bucket".into(),
        prefix: Some("docs/".into()),
        region: Some("us-east-1".into()),
        access_key_id: Some(std::env::var("AWS_ACCESS_KEY_ID").unwrap()),
        secret_access_key: Some(std::env::var("AWS_SECRET_ACCESS_KEY").unwrap()),
        ..Default::default()
    })
    .await?;

let confluence = client
    .sources()
    .create_confluence(ConfluenceSource {
        base_url: Some("https://acme.atlassian.net".into()),
        username: Some("bot@acme.com".into()),
        api_token: Some(std::env::var("CONFLUENCE_API_TOKEN").unwrap()),
        spaces: vec!["ENG".into()],
        ..Default::default()
    })
    .await?;
```

`create_source` accepts any builder directly, and the per-type
`create_web/create_s3/create_gcs/create_google_drive/create_jira/create_confluence/create_file_upload/create_generic`
helpers are thin wrappers over it.

### Jobs

```rust
use vectoramp::StartIngestionRequest;

let dataset = client.datasets().get("dataset-id").await?;

// Create a source and start a job in one call.
let job = dataset
    .ingest_new_source(WebSource {
        start_urls: vec!["https://example.com/releases".into()],
        ..Default::default()
    })
    .await?;

// Start a job from an existing source.
let job = dataset.ingest_source("source-id").await?;

let jobs = client.ingestion().list_jobs(Some("dataset-id"), ()).await?;
let job = client.ingestion().get_job(&job.job_id).await?;
```

### Filesystem upload

`ingest_files` hides the presigned-upload flow: it creates a `file_upload`
source, initializes presigned uploads, PUTs the bytes, and completes the job.

```rust
use std::path::PathBuf;

let job = dataset
    .ingest_files(vec![PathBuf::from("./docs/guide.pdf")], None)
    .await?;
```

## Intelligence / RAG

`ask` defaults `top_k = 5`, `include_sources = true`, and dataset scope `"all"`
when unscoped.

```rust
use vectoramp::AskOptions;

// Unscoped (defaults to all datasets).
let answer = client.ask("What are the key product features?").await?;

// Scoped to a dataset.
let answer = dataset.ask("What are the key product features?").await?;

// Explicit options.
let answer = client
    .ask_with(
        "What changed in the latest release?",
        AskOptions::default().with_all_datasets().with_top_k(8),
    )
    .await?;
```

### Streaming (SSE)

```rust
let mut stream = dataset.ask_stream("Summarize the launch plan").await?;
while let Some(event) = stream.next_event().await? {
    if event.chunk_type == "text" {
        print!("{}", event.content);
    }
}
```

### Sessions

Durable RAG conversations:

```rust
let session = client.intelligence().create_session("Launch planning").await?;

client
    .intelligence()
    .append_message(&session.id, "user", "What is our launch date?")
    .await?;

let messages = client.intelligence().list_messages(&session.id, ()).await?;
let sessions = client.intelligence().list_sessions(()).await?;
let one = client.intelligence().get_session(&session.id).await?;
```

## Errors

Non-2xx responses surface as `Error::Api(ApiError)`.

```rust
match client.datasets().get("missing").await {
    Ok(dataset) => { let _ = dataset; }
    Err(vectoramp::Error::Api(err)) => {
        eprintln!("api error {}: {}", err.status, err.message);
    }
    Err(err) => eprintln!("transport error: {err}"),
}
```

## Method reference

`R = required`, `O = optional`. Pagination arguments accept `()`, `(limit, offset)`,
or a bare `limit`.

### `client.datasets()` — `DatasetService` (and the bound `Dataset` object)

| Method | Args | Returns |
|---|---|---|
| `list(pagination)` | pagination (O) | `DatasetList` |
| `get(id)` | id (R) | `Dataset` |
| `create(req)` | name/builder/request (R) | `Dataset` |
| `delete(id)` / `Dataset::delete()` | id (R) | `()` |
| `search(id, input)` / `Dataset::search(input)` | id (R), text\|vector\|`SearchRequest` (R) | `SearchResponse` |
| `search_with(id, input, opts)` / `Dataset::search_with(input, opts)` | + `SearchOptions` (O) | `SearchResponse` |
| `insert(id, vectors)` / `Dataset::insert(vectors)` (+ `insert_vectors`) | id (R), `Vec<Vector>` (R) | `InsertVectorsResponse` |
| `embed(id, req)` / `Dataset::embed(req)` | id (R), `EmbedRequest` (R) | `EmbedResponse` |
| `add_texts(id, input)` / `Dataset::add_texts(input)` | id (R), texts/docs (R) | `AddTextsResponse` |
| `add_texts_with(id, input, opts)` / `Dataset::add_texts_with(input, opts)` | + `AddTextsOptions` (O) | `AddTextsResponse` |
| `list_documents(id, opts)` / `Dataset::list_documents(opts)` | id (R), `DocumentListOptions` (O) | `DatasetDocumentList` |
| `download_document(id, docId)` / `Dataset::download_document(docId)` | id (R), docId (R) | `Vec<u8>` |
| `ingest_source(id, srcId)` / `Dataset::ingest_source(srcId)` | id (R), srcId (R) | `Job` |
| `ingest_new_source(id, builder)` / `Dataset::ingest_new_source(builder)` | id (R), source builder (R) | `Job` |
| `ingest_files(id, paths, opts)` / `Dataset::ingest_files(paths, opts)` | id (R), paths (R), `IngestFilesOptions` (O) | `Job` |
| `ask(id, query)` / `Dataset::ask(query)` | id (R), query (R) | `AskResponse` |
| `ask_with(id, query, opts)` / `Dataset::ask_with(query, opts)` | + `AskOptions` (O) | `AskResponse` |
| `ask_stream(id, query)` / `Dataset::ask_stream(query)` | id (R), query (R) | `AskStream` |

### `client.ask*` (top level)

| Method | Args | Returns |
|---|---|---|
| `ask(query)` | query (R) | `AskResponse` |
| `ask_with(query, opts)` | query (R), `AskOptions` (O) | `AskResponse` |
| `ask_stream(query)` | query (R) | `AskStream` |

### `client.ingestion()` / `client.sources()` — `IngestionService`

| Method | Args | Returns |
|---|---|---|
| `list_sources(pagination)` | pagination (O) | `SourceList` |
| `get_source(id)` | id (R) | `Source` |
| `create_source(builder)` | source builder (R) | `Source` |
| `create_web/_s3/_gcs/_google_drive/_jira/_confluence/_file_upload/_generic(source)` | typed builder (R) | `Source` |
| `start_job(req)` | `StartIngestionRequest` (R) | `Job` |
| `list_jobs(dataset_id, pagination)` | dataset_id (O), pagination (O) | `JobList` |
| `get_job(id)` | id (R) | `Job` |
| `retry_job(id)` | id (R) | `Job` |
| `ingest_files(dataset_id, paths, opts)` | dataset_id (R), paths (R), opts (O) | `Job` |

### `client.intelligence()` — `IntelligenceService`

| Method | Args | Returns |
|---|---|---|
| `ask(query)` / `ask_with(query, opts)` | query (R), `AskOptions` (O) | `AskResponse` |
| `stream(query, opts)` | query (R), `AskOptions` (O) | `AskStream` |
| `create_session(req)` | title/`CreateSessionRequest` (O) | `IntelligenceSession` |
| `list_sessions(pagination)` | pagination (O) | `SessionList` |
| `get_session(id)` | id (R) | `IntelligenceSession` |
| `delete_session(id)` | id (R) | `()` |
| `append_message(id, role, content)` | id (R), role (R), content (R) | `SessionMessage` |
| `append_message_with(id, req)` | id (R), `AppendMessageRequest` (R) | `SessionMessage` |
| `list_messages(id, pagination)` | id (R), pagination (O) | `MessageList` |

### `client.schedules()` — `ScheduleService`

| Method | Args | Returns |
|---|---|---|
| `list(pagination)` | pagination (O) | `ScheduleList` |
| `get(id)` | id (R) | `Schedule` |
| `create(req)` | `CreateScheduleRequest` (R) | `Schedule` |
| `update(id, req)` | id (R), `UpdateScheduleRequest` (R) | `Schedule` |
| `delete(id)` | id (R) | `()` |
| `trigger(id)` | id (R) | `TriggerScheduleResponse` |

## Development

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

CI runs the format check, clippy, and the full test suite.
