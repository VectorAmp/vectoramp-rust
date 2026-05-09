use serde_json::json;
use vectoramp::{
    sources::IntoCreateSourceRequest, AddTextsResponse, Client, CreateDatasetRequest,
    CreateSourceRequest, FileUploadSource, S3Source, SearchInput, SearchOptions, WebSource,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_client(base_url: &str) -> Client {
    Client::builder()
        .api_key("test-key")
        .base_url(base_url)
        .build()
        .expect("client builds")
}

#[tokio::test]
async fn create_dataset_always_sends_sable_index_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ds_1",
            "name": "docs",
            "dim": 8,
            "metric": "cosine",
            "index_type": "sable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let dataset = client
        .datasets()
        .create(CreateDatasetRequest {
            name: "docs".into(),
            dim: 8,
            metric: Some("cosine".into()),
            ..Default::default()
        })
        .await
        .expect("dataset created");
    assert_eq!(dataset.id(), "ds_1");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["index_type"], "sable");
}

#[tokio::test]
async fn search_defaults_top_k_to_10() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets/ds_1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [],
            "dataset_id": "ds_1",
            "query_time_ms": 1.2
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    client
        .datasets()
        .search("ds_1", "machine learning")
        .await
        .expect("search ok");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["top_k"], 10);
    assert_eq!(body["query_text"], "machine learning");
}

#[tokio::test]
async fn search_with_options_overrides_top_k() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets/ds_1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    client
        .datasets()
        .search_with(
            "ds_1",
            SearchInput::Vector(vec![0.1, 0.2, 0.3]),
            SearchOptions {
                top_k: Some(25),
                include_metadata: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("search ok");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["top_k"], 25);
    assert_eq!(body["include_metadata"], false);
    assert_eq!(body["query"], json!([0.1, 0.2, 0.3]));
}

#[tokio::test]
async fn add_texts_embeds_then_inserts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets/ds_1/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embeddings": [[0.1, 0.2], [0.3, 0.4]]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/datasets/ds_1/insert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"inserted": 2})))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let resp: AddTextsResponse = client
        .datasets()
        .add_texts("ds_1", vec!["hello", "world"])
        .await
        .expect("add_texts ok");
    assert_eq!(resp.inserted, 2);
    assert_eq!(resp.embeddings, 2);

    let received = server.received_requests().await.unwrap();
    let insert_req = received
        .iter()
        .find(|r| r.url.path().ends_with("/insert"))
        .expect("insert request");
    let body: serde_json::Value =
        serde_json::from_slice(&insert_req.body).expect("json body");
    let vectors = body["vectors"].as_array().expect("vectors array");
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0]["id"], "text-1");
    assert_eq!(vectors[1]["id"], "text-2");
    assert_eq!(vectors[0]["metadata"]["text"], "hello");
}

#[tokio::test]
async fn ask_helper_targets_intelligence_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intelligence/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "answer": "VectorAmp is a vector database."
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let resp = client.ask("What is VectorAmp?").await.expect("ask ok");
    assert_eq!(resp.answer, "VectorAmp is a vector database.");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["query"], "What is VectorAmp?");
    assert_eq!(body["stream"], false);
}

#[tokio::test]
async fn api_error_reports_status_and_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/datasets/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"error": "not found"})))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let result = client.datasets().get("missing").await;
    match result {
        Err(vectoramp::Error::Api(api)) => {
            assert_eq!(api.status, 404);
            assert_eq!(api.message, "not found");
        }
        Err(other) => panic!("expected api error, got {other:?}"),
        Ok(_) => panic!("expected error, got ok"),
    }
}

#[test]
fn web_source_builds_default_name_from_first_url() {
    let req: CreateSourceRequest = WebSource {
        start_urls: vec!["https://docs.example.com/guide".into()],
        max_depth: Some(2),
        ..Default::default()
    }
    .into_create_source_request();
    assert_eq!(req.source_type, "web");
    assert_eq!(req.name, "web-docs-example-com");
    assert_eq!(req.config["max_depth"], json!(2));
}

#[test]
fn s3_source_uses_default_sync_mode_and_bucket_name() {
    let req: CreateSourceRequest = S3Source {
        bucket: "My-Bucket".into(),
        ..Default::default()
    }
    .into_create_source_request();
    assert_eq!(req.source_type, "s3");
    assert_eq!(req.name, "s3-my-bucket");
    assert_eq!(req.config["sync_mode"], "incremental");
}

#[test]
fn file_upload_source_defaults() {
    let req: CreateSourceRequest = FileUploadSource::default().into_create_source_request();
    assert_eq!(req.source_type, "file_upload");
    assert_eq!(req.name, "rust-sdk-file-upload");
    assert_eq!(req.config["storage_provider"], "s3");
    assert_eq!(req.config["sync_mode"], "full");
}
