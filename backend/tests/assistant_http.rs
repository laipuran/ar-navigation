use ar_navigation::{
    assistant::{Assistant, AssistantConfig, Llm, LlmConfig},
    http::router,
    navigation::Navigation,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

struct LlmServer {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}

enum Step {
    Message(Value),
    Failure,
    Block {
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        message: Value,
    },
}

#[tokio::test]
async fn responses_have_fresh_request_ids_even_for_invalid_requests() {
    let app = app();
    // 非法请求无需连接LLM。
    let mut ids = Vec::new();
    for body in [r#"{"message":" "}"#, "{"] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/assistant/messages")
                    .header("content-type", "application/json")
                    .header("x-request-id", "untrusted-client-value")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let id = response
            .headers()
            .get("x-request-id")
            .expect("request ID")
            .to_str()
            .unwrap();
        assert!(uuid::Uuid::parse_str(id).is_ok());
        assert_ne!(id, "untrusted-client-value");
        ids.push(id.to_owned());
    }
    assert_ne!(ids[0], ids[1]);
}

impl Drop for LlmServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLogs {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn tracing_correlates_llm_and_background_tools_without_logging_secrets() {
    use tracing::instrument::WithSubscriber;
    let secret = "private-diagnostic-marker";
    let server = scripted_server(vec![
        Step::Message(json!({"role":"assistant","content":secret,"protocol_extension":secret,"tool_calls":[
            tool_call("private-call-id","get_node",json!({"building_id":secret,"node_id":secret})),
            tool_call("private-call-id-2",secret,json!({"input":secret})),
        ]})),
        Step::Message(json!({"role":"assistant","content":secret})),
        Step::Failure,
    ]).await;
    let llm = Llm::new(LlmConfig {
        base_url: server.url.clone(),
        api_key: secret.into(),
        model: secret.into(),
        request_options: json!({}),
    })
    .unwrap();
    let app =
        router(Assistant::new(Navigation::default(), llm, AssistantConfig::default()).unwrap());
    let logs = CapturedLogs(Arc::new(Mutex::new(Vec::new())));
    let subscriber = Arc::new(ar_navigation::telemetry::subscriber(
        tracing_subscriber::EnvFilter::new("warn,ar_navigation=debug"),
        logs.clone(),
    ));
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/messages")
                .header("content-type", "application/json")
                .body(Body::from(json!({"message":secret}).to_string()))
                .unwrap(),
        )
        .with_subscriber(subscriber.clone())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let first_request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let conversation_id = body["conversation_id"].as_str().unwrap();
    let response = app
        .oneshot(
            Request::post("/api/v1/assistant/messages")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"message":secret,"conversation_id":conversation_id}).to_string(),
                ))
                .unwrap(),
        )
        .with_subscriber(subscriber)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let second_request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    for marker in [
        secret,
        conversation_id,
        "private-call-id",
        "private upstream details",
    ] {
        assert!(!text.contains(marker), "敏感内容出现在日志中");
    }
    let tool_log = text
        .lines()
        .find(|line| line.contains("导航工具返回业务错误"))
        .expect("后台工具日志");
    assert!(tool_log.contains(&first_request_id));
    assert!(tool_log.contains("get_node"));
    assert!(tool_log.contains("BUILDING_NOT_FOUND"));
    let llm_log = text
        .lines()
        .find(|line| line.contains("LLM请求失败"))
        .expect("LLM失败日志");
    assert!(llm_log.contains(&second_request_id));
    assert!(llm_log.contains("503"));
    assert!(text.contains("elapsed_ms="));
    assert!(text.contains("call_number=1"));
    assert!(text.contains("MODEL_UNAVAILABLE"));
}

async fn llm_server(replies: Vec<Value>) -> LlmServer {
    scripted_server(replies.into_iter().map(Step::Message).collect()).await
}

async fn scripted_server(replies: Vec<Step>) -> LlmServer {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
    let captured = requests.clone();
    let endpoint = axum::Router::new().route("/chat/completions", axum::routing::post(move |axum::Json(request): axum::Json<Value>| {
        let requests = captured.clone();
        let replies = replies.clone();
        async move {
            requests.lock().unwrap().push(request);
            let step = replies.lock().unwrap().pop_front().expect("unexpected LLM request");
            use axum::response::IntoResponse;
            let message = match step {
                Step::Message(message) => message,
                Step::Failure => return (StatusCode::SERVICE_UNAVAILABLE, axum::Json(json!({"error":{"message":"private upstream details","type":"unavailable"}}))).into_response(),
                Step::Block { started, release, message } => {
                    started.notify_one();
                    release.notified().await;
                    message
                }
            };
            axum::Json(json!({"choices": [{"finish_reason": if message["tool_calls"].is_array() {"tool_calls"} else {"stop"}, "message": message}]})).into_response()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, endpoint).await.unwrap();
    });
    LlmServer {
        url,
        requests,
        task,
    }
}

fn configured_app(server: &LlmServer, config: AssistantConfig) -> axum::Router {
    let llm = Llm::new(LlmConfig {
        base_url: server.url.clone(),
        api_key: "test".into(),
        model: "test-llm".into(),
        request_options: json!({}),
    })
    .unwrap();
    let building = ar_navigation::navigation::loader::load_building_json(
        ar_navigation::model::BuildingSummary {
            id: ar_navigation::model::BuildingId("demo".into()),
            name: "演示楼".into(),
        },
        include_str!("fixtures/teaching-a.json"),
    )
    .unwrap();
    router(Assistant::new(Navigation::new([building]).unwrap(), llm, config).unwrap())
}

async fn send(app: &axum::Router, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/messages")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}

#[tokio::test]
async fn continues_conversation_through_real_navigation_tools_and_preserves_llm_fields() {
    let call = json!({"role":"assistant", "content":null, "protocol_extension":"preserve-me", "tool_calls":[
        {"id":"route-1", "type":"function", "function":{"name":"find_route", "arguments":r#"{"building_id":"demo","from":"gate","to":"upper"}"#}}
    ]});
    let server = llm_server(vec![
        json!({"role":"assistant","content":"你在哪个入口？"}),
        call.clone(),
        json!({"role":"assistant","content":"路线共14米。"}),
    ])
    .await;
    let app = configured_app(&server, AssistantConfig::default());
    let (status, first) = send(&app, json!({"message":"去201"})).await;
    assert_eq!(status, StatusCode::OK);
    let id = first["conversation_id"].as_str().unwrap();
    let (status, next) = send(
        &app,
        json!({"conversation_id":id,"message":"我在演示楼东门"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(next["conversation_id"], id);
    assert_eq!(next["reply"], "路线共14米。");
    let requests = server.requests.lock().unwrap();
    let history = requests[2]["messages"].as_array().unwrap();
    assert_eq!(history[1]["content"], "去201");
    assert_eq!(history[2]["content"], "你在哪个入口？");
    assert_eq!(history[4], call);
    assert_eq!(history[5]["tool_call_id"], "route-1");
    let result: Value = serde_json::from_str(history[5]["content"].as_str().unwrap()).unwrap();
    assert_eq!(result["data"]["distance_m"], 14.0);
    assert_eq!(
        result["data"]["path"],
        json!(["gate", "junction", "stairs-1", "stairs-2", "upper"])
    );
    assert_eq!(
        result["data"]["steps"].as_array().unwrap().last().unwrap()["type"],
        "arrive"
    );
}

#[tokio::test]
async fn upstream_failure_is_not_retried_and_does_not_commit_half_a_turn() {
    let server = scripted_server(vec![
        Step::Message(json!({"role":"assistant","content":"请确认入口"})),
        Step::Failure,
        Step::Message(json!({"role":"assistant","content":"请补充目的地"})),
    ])
    .await;
    let app = configured_app(&server, AssistantConfig::default());
    let (_, first) = send(&app, json!({"message":"找教室"})).await;
    let id = &first["conversation_id"];
    let (status, error) = send(&app, json!({"conversation_id":id,"message":"东门"})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(error["error"]["code"], "MODEL_UNAVAILABLE");
    assert!(!error.to_string().contains("private upstream"));
    let (status, _) = send(&app, json!({"conversation_id":id,"message":"东门"})).await;
    assert_eq!(status, StatusCode::OK);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1]["messages"], requests[2]["messages"]);
}

#[tokio::test]
async fn limits_global_concurrency_without_leaking_new_session_reservations() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let server = scripted_server(vec![
        Step::Block {
            started: started.clone(),
            release: release.clone(),
            message: json!({"role":"assistant","content":"请确认入口"}),
        },
        Step::Message(json!({"role":"assistant","content":"另一个会话"})),
    ])
    .await;
    let config = AssistantConfig {
        max_concurrent_turns: 1,
        max_sessions: 2,
        ..Default::default()
    };
    let app = configured_app(&server, config);
    let first = tokio::spawn({
        let app = app.clone();
        async move { send(&app, json!({"message":"找教室"})).await }
    });
    started.notified().await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/messages")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message":"另一个请求"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "30");
    release.notify_one();
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    assert_eq!(
        send(&app, json!({"message":"重试"})).await.0,
        StatusCode::OK
    );
}

async fn advance_time(seconds: u64) {
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(seconds)).await;
    tokio::time::resume();
}

#[tokio::test]
async fn ttl_refreshes_after_success_and_expired_sessions_release_capacity() {
    let reply = json!({"role":"assistant","content":"请补充位置"});
    let server = llm_server(vec![reply.clone(), reply.clone(), reply.clone(), reply]).await;
    let config = AssistantConfig {
        session_ttl: std::time::Duration::from_secs(30),
        max_sessions: 1,
        ..Default::default()
    };
    let app = configured_app(&server, config);
    let (_, first) = send(&app, json!({"message":"找教室"})).await;
    let id = &first["conversation_id"];
    assert_eq!(
        send(&app, json!({"message":"新会话"})).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    advance_time(20).await;
    assert_eq!(
        send(&app, json!({"conversation_id":id,"message":"东门"}))
            .await
            .0,
        StatusCode::OK
    );
    advance_time(20).await;
    assert_eq!(
        send(&app, json!({"conversation_id":id,"message":"201"}))
            .await
            .0,
        StatusCode::OK
    );
    advance_time(30).await;
    let (status, body) = send(&app, json!({"conversation_id":id,"message":"继续"})).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"]["code"], "CONVERSATION_UNAVAILABLE");
    let (status, next) = send(&app, json!({"message":"重新开始"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(next["conversation_id"], *id);
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn busy_sessions_are_protected_from_expiry_and_other_sessions_can_progress() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let reply = json!({"role":"assistant","content":"请补充位置"});
    let server = scripted_server(vec![
        Step::Message(reply.clone()),
        Step::Block {
            started: started.clone(),
            release: release.clone(),
            message: reply.clone(),
        },
        Step::Message(reply.clone()),
        Step::Message(reply),
    ])
    .await;
    let app = configured_app(
        &server,
        AssistantConfig {
            session_ttl: std::time::Duration::from_secs(1),
            ..Default::default()
        },
    );
    let (_, first) = send(&app, json!({"message":"找教室"})).await;
    let id = first["conversation_id"].clone();
    let running = tokio::spawn({
        let app = app.clone();
        let id = id.clone();
        async move { send(&app, json!({"conversation_id":id,"message":"被取消的消息"})).await }
    });
    started.notified().await;
    advance_time(2).await;
    let (status, error) = send(&app, json!({"conversation_id":id,"message":"不能并发"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "CONVERSATION_BUSY");
    // A separate conversation can progress without waiting for this one.
    assert_eq!(
        send(&app, json!({"message":"另一个会话"})).await.0,
        StatusCode::OK
    );
    release.notify_one();
    assert_eq!(running.await.unwrap().0, StatusCode::OK);
    let (status, _) = send(&app, json!({"conversation_id":id,"message":"下一步"})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn cancelled_turn_releases_busy_state_and_preserves_committed_history() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let reply = json!({"role":"assistant","content":"请补充位置"});
    let server = scripted_server(vec![
        Step::Message(reply.clone()),
        Step::Block {
            started: started.clone(),
            release: release.clone(),
            message: reply.clone(),
        },
        Step::Message(reply),
    ])
    .await;
    let app = configured_app(&server, AssistantConfig::default());
    let (_, first) = send(&app, json!({"message":"找教室"})).await;
    let id = first["conversation_id"].clone();
    let running = tokio::spawn({
        let app = app.clone();
        let id = id.clone();
        async move { send(&app, json!({"conversation_id":id,"message":"被取消的消息"})).await }
    });
    started.notified().await;
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    let (status, _) = send(&app, json!({"conversation_id":id,"message":"重试消息"})).await;
    assert_eq!(status, StatusCode::OK);
    let requests = server.requests.lock().unwrap();
    let history = requests[2]["messages"].as_array().unwrap();
    assert_eq!(history.len(), 4);
    assert_eq!(history[3]["content"], "重试消息");
    assert!(!history.iter().any(|m| m["content"] == "被取消的消息"));
}

#[tokio::test]
async fn swagger_describes_session_lifecycle_and_all_public_errors() {
    let response = app()
        .oneshot(
            Request::get("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let operation = &document["paths"]["/api/v1/assistant/messages"]["post"];
    for status in ["200", "400", "409", "410", "429", "502", "504"] {
        assert!(
            operation["responses"].get(status).is_some(),
            "missing {status}"
        );
    }
    assert!(
        operation["responses"]["429"]["headers"]
            .get("Retry-After")
            .is_some()
    );
    let response = app()
        .oneshot(Request::get("/swagger-ui/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("swagger-ui"));
}

#[tokio::test]
async fn timeout_discards_new_session_and_releases_capacity() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let reply = json!({"role":"assistant","content":"请补充位置"});
    let server = scripted_server(vec![
        Step::Block {
            started,
            release,
            message: reply.clone(),
        },
        Step::Message(reply),
    ])
    .await;
    let app = configured_app(
        &server,
        AssistantConfig {
            turn_timeout: std::time::Duration::from_millis(100),
            max_sessions: 1,
            ..Default::default()
        },
    );
    let (status, body) = send(&app, json!({"message":"找教室"})).await;
    assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(body["error"]["code"], "REQUEST_TIMEOUT");
    assert!(body.get("conversation_id").is_none());
    assert_eq!(
        send(&app, json!({"message":"重试"})).await.0,
        StatusCode::OK
    );
}

fn tool_call(id: &str, name: &str, arguments: Value) -> Value {
    json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments.to_string()}})
}

#[tokio::test]
async fn tool_errors_are_also_bounded_before_returning_to_llm() {
    let server = llm_server(vec![json!({"role":"assistant","content":null,"tool_calls":[
        tool_call("large", "get_node", json!({"building_id":"x".repeat(70_000),"node_id":"gate"}))
    ]}), json!({"role":"assistant","content":"请确认楼栋"})]).await;
    let app = configured_app(&server, AssistantConfig::default());
    assert_eq!(
        send(&app, json!({"message":"找教室"})).await.0,
        StatusCode::OK
    );
    let requests = server.requests.lock().unwrap();
    let content = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(content.len() <= 64 * 1024);
    let result: Value = serde_json::from_str(content).unwrap();
    assert_eq!(result["error"]["code"], "RESULT_TOO_LARGE");
}

#[tokio::test]
async fn tool_batch_preserves_candidates_and_returns_structured_business_errors() {
    let calls = vec![
        tool_call("list", "list_buildings", json!({})),
        tool_call(
            "search",
            "search_nodes",
            json!({"building_id":"demo","text":"101","kind":"room","floor_id":1}),
        ),
        tool_call(
            "get",
            "get_node",
            json!({"building_id":"demo","node_id":"room"}),
        ),
        tool_call(
            "no-route",
            "find_route",
            json!({"building_id":"demo","from":"gate","to":"isolated"}),
        ),
        tool_call(
            "missing-node",
            "get_node",
            json!({"building_id":"demo","node_id":"missing"}),
        ),
        tool_call("extra", "list_buildings", json!({"unexpected":true})),
        tool_call("unknown", "run_shell", json!({"command":"not executed"})),
        tool_call(
            "invalid-kind",
            "search_nodes",
            json!({"building_id":"demo","kind":"unknown"}),
        ),
        tool_call(
            "missing-building",
            "get_node",
            json!({"building_id":"missing","node_id":"gate"}),
        ),
        tool_call(
            "blank",
            "search_nodes",
            json!({"building_id":"demo","text":"  "}),
        ),
    ];
    let server = llm_server(vec![
        json!({"role":"assistant","content":null,"tool_calls":calls}),
        json!({"role":"assistant","content":"有两个101教室，请确认目的地；封闭教室无法到达。"}),
    ])
    .await;
    let app = configured_app(&server, AssistantConfig::default());
    assert_eq!(
        send(&app, json!({"message":"查询演示楼"})).await.0,
        StatusCode::OK
    );
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 4);
    let history = requests[1]["messages"].as_array().unwrap();
    let results: Vec<Value> = history
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(results[0]["data"][0]["id"], "demo");
    assert_eq!(results[1]["data"].as_array().unwrap().len(), 2);
    assert_eq!(results[2]["data"]["kind"]["name"], "101");
    for (index, code) in [
        (3, "NO_ROUTE"),
        (4, "NODE_NOT_FOUND"),
        (5, "INVALID_ARGUMENTS"),
        (6, "UNKNOWN_TOOL"),
        (7, "INVALID_ARGUMENTS"),
        (8, "BUILDING_NOT_FOUND"),
        (9, "INVALID_INPUT"),
    ] {
        assert_eq!(results[index]["ok"], false);
        assert_eq!(results[index]["error"]["code"], code);
    }
}

#[tokio::test]
async fn exhausted_tool_budget_aborts_without_publishing_a_session() {
    let server = llm_server(vec![
        json!({"role":"assistant","content":null,"tool_calls":[
            tool_call("one","unknown",json!({})), tool_call("two","list_buildings",json!({}))
        ]}),
        json!({"role":"assistant","content":"请提供楼栋"}),
    ])
    .await;
    let app = configured_app(
        &server,
        AssistantConfig {
            max_tool_calls: 1,
            max_sessions: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        send(&app, json!({"message":"找教室"})).await.0,
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    assert_eq!(
        send(&app, json!({"message":"新对话"})).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn exhausted_llm_budget_does_not_make_an_extra_request() {
    let server = llm_server(vec![json!({"role":"assistant","content":null,"tool_calls":[tool_call("one","list_buildings",json!({}))]})]).await;
    let app = configured_app(
        &server,
        AssistantConfig {
            max_llm_calls: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        send(&app, json!({"message":"有哪些楼栋"})).await.0,
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn history_limit_is_explicit_and_does_not_consume_session_capacity() {
    let server = llm_server(vec![json!({"role":"assistant","content":"请提供楼栋"})]).await;
    let app = configured_app(
        &server,
        AssistantConfig {
            max_history_bytes: 8192,
            max_sessions: 1,
            ..Default::default()
        },
    );
    let (status, body) = send(&app, json!({"message":"字".repeat(3000)})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "CONVERSATION_LIMIT_REACHED");
    assert_eq!(server.requests.lock().unwrap().len(), 0);
    assert_eq!(
        send(&app, json!({"message":"找教室"})).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn malformed_requests_share_the_json_error_contract() {
    for body in [
        r#"{"message":5}"#.to_owned(),
        r#"{"message":"a","extra":true}"#.into(),
        "{".into(),
        "{}".into(),
        json!({"message":"x".repeat(4001)}).to_string(),
        json!({"message":"a","conversation_id":""}).to_string(),
        json!({"message":"x".repeat(40_000)}).to_string(),
    ] {
        let response = app()
            .oneshot(
                Request::post("/api/v1/assistant/messages")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["error"]["code"], "INVALID_REQUEST");
    }
    let (status, body) = send(
        &app(),
        json!({"message":"继续","conversation_id":"unknown"}),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"]["code"], "CONVERSATION_UNAVAILABLE");
}

fn app() -> axum::Router {
    let llm = Llm::new(LlmConfig {
        base_url: "http://127.0.0.1:1".into(),
        api_key: "test".into(),
        model: "test-llm".into(),
        request_options: json!({}),
    })
    .unwrap();
    router(Assistant::new(Navigation::default(), llm, AssistantConfig::default()).unwrap())
}

#[tokio::test]
async fn rejects_blank_messages_with_contract_error() {
    let response = app()
        .oneshot(
            Request::post("/api/v1/assistant/messages")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message":"  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["error"]["code"], "INVALID_REQUEST");
}
