//! Official Rust client for the VectorAmp public API.
//!
//! Default API base URL: `https://api.vectoramp.com`. Authentication uses the
//! `X-API-Key` header. The default transport is JSON over HTTPS with a small
//! [`Transport`] trait so other transports (gRPC, mocks) can be plugged in.
//!
//! Public dataset creation always uses the SABLE index type; the SDK does not
//! expose an index-type option.
//!
//! # Quick start
//!
//! ```no_run
//! use vectoramp::{Client, CreateDatasetRequest, EmbeddingConfig};
//!
//! # async fn run() -> vectoramp::Result<()> {
//! let client = Client::new(std::env::var("VECTORAMP_API_KEY").unwrap());
//!
//! let dataset = client
//!     .datasets()
//!     .create(CreateDatasetRequest {
//!         name: "product-docs".into(),
//!         dim: 2560,
//!         metric: Some("cosine".into()),
//!         embedding: Some(EmbeddingConfig {
//!             provider: Some("vectoramp".into()),
//!             model: Some("VectorAmp-Embedding-2560".into()),
//!         }),
//!         ..Default::default()
//!     })
//!     .await?;
//!
//! dataset.add_texts(vec!["VectorAmp is a high-performance vector database."]).await?;
//!
//! let answer = dataset.ask("What is VectorAmp?").await?;
//! println!("{}", answer.answer);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod datasets;
pub mod errors;
pub mod ingestion;
pub mod intelligence;
pub mod schedules;
pub mod sources;
pub mod transport;
pub mod types;

pub use client::{Client, ClientBuilder};
pub use datasets::{AddTextsOptions, Dataset, DatasetService, SearchInput, SearchOptions};
pub use errors::{ApiError, Error, Result};
pub use ingestion::{DocumentListOptions, IngestFilesOptions, IngestionService};
pub use intelligence::{AskOptions, AskStream, IntelligenceService, StreamEvent};
pub use schedules::ScheduleService;
pub use sources::{
    FileUploadSource, GcsSource, GenericSource, GoogleDriveSource, IntoCreateSourceRequest,
    JiraSource, S3Source, WebSelectors, WebSource,
};
pub use transport::{Request, Response, Transport};
pub use types::*;

/// Default VectorAmp API base URL used when none is supplied to the builder.
pub const DEFAULT_BASE_URL: &str = "https://api.vectoramp.com";

/// User-agent string sent with every request.
pub const USER_AGENT: &str = concat!("vectoramp-rust/", env!("CARGO_PKG_VERSION"));
