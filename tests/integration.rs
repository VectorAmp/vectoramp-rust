use serde_json::json;
use vectoramp::{
    sources::IntoCreateSourceRequest, AddTextsResponse, Client, ConfluenceSource,
    CreateDatasetRequest, CreateScheduleRequest, CreateSourceRequest, FileUploadSource, S3Source,
    SearchInput, SearchOptions, UpdateScheduleRequest, Vector, VectorId, WebSource,
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
            dim: Some(8),
            metric: Some("cosine".into()),
            ..Default::default()
        })
        .await
        .expect("dataset created");
    assert_eq!(dataset.id(), "ds_1");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["index_type"], "sable");
    // The request field is `dim`, never `dimension`.
    assert_eq!(body["dim"], 8);
    assert!(body.get("dimension").is_none());
}

#[tokio::test]
async fn create_dataset_minimal_name_only_infers_defaults() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ds_min",
            "name": "docs",
            "dim": 2560,
            "metric": "cosine",
            "index_type": "sable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    // Only a name — everything else defaulted/inferred by the SDK.
    let dataset = client.datasets().create("docs").await.expect("created");
    assert_eq!(dataset.id(), "ds_min");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["name"], "docs");
    assert_eq!(body["index_type"], "sable");
    assert_eq!(body["dim"], 2560);
    assert_eq!(body["metric"], "cosine");
    assert_eq!(body["embedding"]["provider"], "vectoramp");
    assert_eq!(body["embedding"]["model"], "VectorAmp-Embedding-4B");
    // Hybrid is not sent unless explicitly requested.
    assert!(body.get("hybrid").is_none());
}

#[tokio::test]
async fn create_dataset_hybrid_sends_hybrid_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ds_hy",
            "name": "docs",
            "dim": 2560,
            "metric": "cosine",
            "index_type": "sable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let dataset = client
        .datasets()
        .create(CreateDatasetRequest::builder("docs").hybrid(true))
        .await
        .expect("created");
    assert_eq!(dataset.id(), "ds_hy");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["hybrid"], true);
    assert_eq!(body["index_type"], "sable");
    assert_eq!(body["dim"], 2560);
}

#[tokio::test]
async fn create_dataset_openai_large_infers_3072() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ds_oa",
            "name": "docs",
            "dim": 3072,
            "metric": "cosine",
            "index_type": "sable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    client
        .datasets()
        .create(CreateDatasetRequest::builder("docs").openai("large"))
        .await
        .expect("created");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["dim"], 3072);
    assert_eq!(body["embedding"]["provider"], "openai");
    assert_eq!(body["embedding"]["model"], "text-embedding-3-large");
}

#[tokio::test]
async fn create_dataset_unknown_model_without_dim_errors() {
    let client = test_client("http://127.0.0.1:1");
    let result = client
        .datasets()
        .create(
            CreateDatasetRequest::builder("docs").embedding(vectoramp::EmbeddingConfig {
                provider: Some("acme".into()),
                model: Some("acme-embed-9000".into()),
                ..Default::default()
            }),
        )
        .await;
    match result {
        Err(vectoramp::Error::InvalidInput(msg)) => assert!(msg.contains("dim")),
        Err(other) => panic!("expected invalid input error, got {other:?}"),
        Ok(_) => panic!("expected error for unknown model without dim, got ok"),
    }
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
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
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
                rerank: Some(vectoramp::Rerank::Config(vectoramp::RerankConfig {
                    enabled: true,
                    ..Default::default()
                })),
                ..Default::default()
            },
        )
        .await
        .expect("search ok");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["top_k"], 25);
    assert_eq!(body["include_metadata"], false);
    assert_eq!(body["query"], json!([0.1, 0.2, 0.3]));
    assert_eq!(body["rerank"], json!({"enabled": true}));
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
    let body: serde_json::Value = serde_json::from_slice(&insert_req.body).expect("json body");
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
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
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

#[tokio::test]
async fn schedules_crud_and_trigger() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/ingestion/schedules"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "schedules": [{"id": "sch_1", "cron": "0 * * * *", "enabled": true}],
            "total": 1,
            "limit": 10,
            "offset": 0
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/ingestion/schedules/sch_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "sch_1", "cron": "0 * * * *", "enabled": true
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/ingestion/schedules"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "sch_2", "cron": "0 0 * * *", "enabled": true
        })))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/ingestion/schedules/sch_2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "sch_2", "enabled": false
        })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/ingestion/schedules/sch_2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted": true})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/ingestion/schedules/sch_1/trigger"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({"job_id": "job_42"})))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());

    let page = client.schedules().list((10, 0)).await.expect("list");
    assert_eq!(page.total, 1);
    assert_eq!(page.schedules[0].id, "sch_1");

    let one = client.schedules().get("sch_1").await.expect("get");
    assert_eq!(one.id, "sch_1");

    let created = client
        .schedules()
        .create(CreateScheduleRequest {
            source_id: "src_1".into(),
            dataset_id: "ds_1".into(),
            cron: "0 0 * * *".into(),
            timezone: Some("UTC".into()),
            ..Default::default()
        })
        .await
        .expect("create");
    assert_eq!(created.id, "sch_2");

    let updated = client
        .schedules()
        .update(
            "sch_2",
            UpdateScheduleRequest {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("update");
    assert!(!updated.enabled);

    client.schedules().delete("sch_2").await.expect("delete");

    let trig = client.schedules().trigger("sch_1").await.expect("trigger");
    assert_eq!(trig.job_id.as_deref(), Some("job_42"));
}

#[test]
fn file_upload_source_defaults() {
    let req: CreateSourceRequest = FileUploadSource::default().into_create_source_request();
    assert_eq!(req.source_type, "file_upload");
    assert_eq!(req.name, "rust-sdk-file-upload");
    assert_eq!(req.config["storage_provider"], "s3");
    assert_eq!(req.config["sync_mode"], "full");
}

#[test]
fn confluence_source_builds_config_and_default_name() {
    let req: CreateSourceRequest = ConfluenceSource {
        base_url: Some("https://acme.atlassian.net".into()),
        username: Some("bot@acme.com".into()),
        api_token: Some("token".into()),
        spaces: vec!["ENG".into(), "DOCS".into()],
        ..Default::default()
    }
    .into_create_source_request();

    assert_eq!(req.source_type, "confluence");
    assert_eq!(req.config["type"], "confluence");
    assert_eq!(req.config["auth_mode"], "basic");
    assert_eq!(req.config["sync_mode"], "incremental");
    assert_eq!(req.config["include_attachments"], false);
    assert_eq!(req.config["base_url"], "https://acme.atlassian.net");
    assert_eq!(req.config["username"], "bot@acme.com");
    assert_eq!(req.config["spaces"], json!(["ENG", "DOCS"]));
    // Name derives from the first space.
    assert_eq!(req.name, "confluence-eng");
}

#[test]
fn confluence_source_name_falls_back_to_host_then_cloud_id() {
    let from_host: CreateSourceRequest = ConfluenceSource {
        base_url: Some("https://acme.atlassian.net".into()),
        ..Default::default()
    }
    .into_create_source_request();
    assert_eq!(from_host.name, "confluence-acme-atlassian-net");

    let from_cloud: CreateSourceRequest = ConfluenceSource {
        cloud_id: Some("cloud-123".into()),
        ..Default::default()
    }
    .into_create_source_request();
    assert_eq!(from_cloud.name, "confluence-cloud-123");
}

#[tokio::test]
async fn create_confluence_source_posts_to_ingestion_sources() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ingestion/sources"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "src_conf",
            "name": "confluence-eng",
            "source_type": "confluence"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let source = client
        .sources()
        .create_confluence(ConfluenceSource {
            cloud_id: Some("cloud-1".into()),
            spaces: vec!["ENG".into()],
            ..Default::default()
        })
        .await
        .expect("created");
    assert_eq!(source.identifier(), Some("src_conf"));

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["source_type"], "confluence");
    assert_eq!(body["config"]["cloud_id"], "cloud-1");
}

#[tokio::test]
async fn insert_uses_insert_endpoint_and_preserves_numeric_ids() {
    let server = MockServer::start().await;
    // The endpoint is /insert (never /vectors).
    Mock::given(method("POST"))
        .and(path("/datasets/ds_1/insert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"inserted": 2})))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let resp = client
        .datasets()
        .insert(
            "ds_1",
            vec![
                Vector::new(42, vec![0.1, 0.2]),
                Vector::new("doc-9", vec![0.3, 0.4]),
            ],
        )
        .await
        .expect("insert ok");
    assert_eq!(resp.inserted, 2);

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    let vectors = body["vectors"].as_array().expect("vectors array");
    // Numeric id serialized as a JSON number, not a string.
    assert_eq!(vectors[0]["id"], json!(42));
    assert!(vectors[0]["id"].is_number());
    // String ids stay strings.
    assert_eq!(vectors[1]["id"], json!("doc-9"));
    assert!(vectors[1]["id"].is_string());
}

#[test]
fn vector_id_serializes_int_as_number_and_str_as_string() {
    let int_id = serde_json::to_value(VectorId::Int(7)).unwrap();
    assert_eq!(int_id, json!(7));
    assert!(int_id.is_number());

    let str_id = serde_json::to_value(VectorId::from("abc")).unwrap();
    assert_eq!(str_id, json!("abc"));
    assert!(str_id.is_string());

    // From<integer> conversions land on the numeric variant.
    let from_u64: VectorId = 99u64.into();
    assert_eq!(serde_json::to_value(from_u64).unwrap(), json!(99));
}

#[tokio::test]
async fn list_datasets_defaults_pagination() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "datasets": [],
            "total": 0,
            "limit": 50,
            "offset": 0
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    // No pagination args required.
    client.datasets().list(()).await.expect("list ok");

    let received = server.received_requests().await.unwrap();
    let query = received[0].url.query().unwrap_or("");
    assert!(query.contains("limit=50"), "default limit applied: {query}");
}

#[tokio::test]
async fn intelligence_sessions_lifecycle() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intelligence/sessions"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "sess_1",
            "title": "Launch planning",
            "status": "active"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/intelligence/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sessions": [{"id": "sess_1", "title": "Launch planning"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/intelligence/sessions/sess_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "sess_1", "title": "Launch planning", "status": "active"
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/intelligence/sessions/sess_1/messages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "msg_1", "session_id": "sess_1", "role": "user", "content": "Hello"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/intelligence/sessions/sess_1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "messages": [{"id": "msg_1", "role": "user", "content": "Hello"}]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());

    let created = client
        .intelligence()
        .create_session("Launch planning")
        .await
        .expect("create session");
    assert_eq!(created.id, "sess_1");

    let sessions = client
        .intelligence()
        .list_sessions(())
        .await
        .expect("list sessions");
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(sessions.sessions[0].id, "sess_1");

    let one = client
        .intelligence()
        .get_session("sess_1")
        .await
        .expect("get session");
    assert_eq!(one.id, "sess_1");

    let msg = client
        .intelligence()
        .append_message("sess_1", "user", "Hello")
        .await
        .expect("append message");
    assert_eq!(msg.id, "msg_1");
    assert_eq!(msg.role, "user");

    let messages = client
        .intelligence()
        .list_messages("sess_1", ())
        .await
        .expect("list messages");
    assert_eq!(messages.messages.len(), 1);
    assert_eq!(messages.messages[0].content, "Hello");

    // Verify the create session body carried the title.
    let received = server.received_requests().await.unwrap();
    let create_req = received
        .iter()
        .find(|r| r.method.as_str() == "POST" && r.url.path() == "/intelligence/sessions")
        .expect("create request");
    let body: serde_json::Value = serde_json::from_slice(&create_req.body).expect("json body");
    assert_eq!(body["title"], "Launch planning");
}

#[tokio::test]
async fn dataset_ask_stream_targets_intelligence_with_stream_true() {
    let server = MockServer::start().await;
    let sse = "event: chunk\ndata: {\"chunk_type\":\"text\",\"content\":\"Hi\"}\n\n";
    Mock::given(method("POST"))
        .and(path("/intelligence/query"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let mut stream = client
        .datasets()
        .ask_stream("ds_1", "hello")
        .await
        .expect("stream opens");
    let mut text = String::new();
    while let Some(event) = stream.next_event().await.expect("event") {
        if event.chunk_type == "text" {
            text.push_str(&event.content);
        }
    }
    assert_eq!(text, "Hi");

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["stream"], true);
    assert_eq!(body["dataset_id"], "ds_1");
}

#[tokio::test]
async fn delete_vectors_sends_ids_and_write_concern() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/datasets/ds_1/vectors"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "deleted": 2,
            "dataset_id": "ds_1"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let resp = client
        .datasets()
        .delete_vectors_with_write_concern(
            "ds_1",
            vec![VectorId::from("doc-1"), VectorId::from(42)],
            Some("majority"),
        )
        .await
        .expect("delete vectors ok");
    assert_eq!(resp.deleted, 2);

    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).expect("json body");
    assert_eq!(body["ids"], json!(["doc-1", 42]));
    assert_eq!(body["write_concern"], "majority");
}

#[tokio::test]
async fn create_with_openai_api_key_puts_secret_then_sets_secret_ref() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/org-secrets/emb%3Aopenai%3Aapi_key"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ds_oa_secret",
            "name": "docs",
            "dim": 1536,
            "metric": "cosine",
            "index_type": "sable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let dataset = client
        .datasets()
        .create_with_openai_api_key("docs", "sk-test")
        .await
        .expect("created");
    assert_eq!(dataset.id(), "ds_oa_secret");

    let received = server.received_requests().await.unwrap();
    let secret_req = received
        .iter()
        .find(|r| r.url.path() == "/org-secrets/emb%3Aopenai%3Aapi_key")
        .expect("secret request");
    let secret_body: serde_json::Value =
        serde_json::from_slice(&secret_req.body).expect("json body");
    assert_eq!(secret_body["value"], "sk-test");

    let create_req = received
        .iter()
        .find(|r| r.url.path() == "/datasets")
        .expect("create request");
    let body: serde_json::Value = serde_json::from_slice(&create_req.body).expect("json body");
    assert_eq!(body["embedding"]["provider"], "openai");
    assert_eq!(body["embedding"]["model"], "text-embedding-3-small");
    assert_eq!(
        body["embedding"]["secret_ref"],
        vectoramp::OPENAI_API_KEY_SECRET_REF
    );
    assert_eq!(body["dim"], 1536);
}
