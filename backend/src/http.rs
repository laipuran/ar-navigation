//! HTTP DTO handling and generated OpenAPI documentation.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, MatchedPath, Request, State, rejection::JsonRejection},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Serialize;
use tracing::Instrument;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::assistant::{Assistant, AssistantError, MessageRequest, MessageResponse};

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    error: ErrorDetail,
}
#[derive(Serialize, ToSchema)]
pub struct ErrorDetail {
    code: String,
    message: String,
}

impl IntoResponse for AssistantError {
    fn into_response(self) -> Response {
        let code = self.code();
        if matches!(self, Self::InternalError) {
            tracing::error!(error_code = code, "对话执行失败");
        } else {
            tracing::warn!(error_code = code, "对话请求未完成");
        }
        let status = match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::ModelUnavailable => StatusCode::BAD_GATEWAY,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ConversationUnavailable => StatusCode::GONE,
            Self::ConversationBusy | Self::ConversationLimitReached => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::RequestTimeout => StatusCode::GATEWAY_TIMEOUT,
        };
        let mut response = (
            status,
            Json(ErrorResponse {
                error: ErrorDetail {
                    code: code.into(),
                    message: self.to_string(),
                },
            }),
        )
            .into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert("retry-after", "30".parse().expect("static header"));
        }
        response
    }
}

/// 发送一轮寻路消息。
///
/// 首轮省略 conversation_id，续聊只传本轮消息与返回的 ID。同会话不能并发。
/// 会话在成功响应后刷新空闲 TTL，默认 30 分钟；服务重启后失效。
/// 系统失败不提交半轮历史。网络结果不明时不要自动重发。
/// conversation_id 是访问凭据，请勿放进 URL 或日志。
/// 所有响应携带独立的X-Request-ID，可用来关联后端日志。
#[utoipa::path(post, path = "/api/v1/assistant/messages", tag = "Assistant",
    request_body = MessageRequest,
    responses(
        (status = 200, description = "回答、追问或说明无法到达", body = MessageResponse),
        (status = 400, description = "INVALID_REQUEST：非法 JSON、未知字段、空白消息或超过输入限制", body = ErrorResponse),
        (status = 409, description = "CONVERSATION_BUSY：稍后重试；CONVERSATION_LIMIT_REACHED：开始新对话", body = ErrorResponse),
        (status = 410, description = "CONVERSATION_UNAVAILABLE：会话失效，确认后开始新对话", body = ErrorResponse),
        (status = 429, description = "RATE_LIMITED：服务容量已满", body = ErrorResponse, headers(("Retry-After" = u64, description = "建议等待的秒数"))),
        (status = 502, description = "MODEL_UNAVAILABLE：LLM 失败、协议异常或调用预算耗尽", body = ErrorResponse),
        (status = 500, description = "INTERNAL_ERROR：内部执行失败", body = ErrorResponse),
        (status = 504, description = "REQUEST_TIMEOUT：本轮或 LLM 请求超时", body = ErrorResponse)
    ))]
async fn send(
    State(assistant): State<Assistant>,
    request: Result<Json<MessageRequest>, JsonRejection>,
) -> Result<Json<MessageResponse>, AssistantError> {
    let Json(request) = request.map_err(|_| AssistantError::InvalidRequest)?;
    assistant.send(request).await.map(Json)
}

#[derive(OpenApi)]
#[openapi(
    paths(send),
    components(schemas(MessageRequest, MessageResponse, ErrorResponse, ErrorDetail))
)]
struct ApiDoc;

pub fn router(assistant: Assistant) -> Router {
    Router::new()
        .route("/api/v1/assistant/messages", post(send))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(middleware::from_fn(trace_request))
        .with_state(assistant)
}

async fn trace_request(request: Request, next: Next) -> Response {
    // 独立生成诊断标识，不信任客户端值，也不使用会话凭据。
    let request_id = uuid::Uuid::new_v4().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str);
    let span = tracing::info_span!("http.request", request_id = %request_id, method = %request.method(), route);
    async move {
        let started = std::time::Instant::now();
        tracing::debug!("收到HTTP请求");
        let mut response = next.run(request).await;
        let status = response.status().as_u16();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        if response.status().is_server_error() {
            tracing::error!(status, elapsed_ms, "HTTP请求完成");
        } else if response.status().is_client_error() {
            tracing::warn!(status, elapsed_ms, "HTTP请求完成");
        } else {
            tracing::info!(status, elapsed_ms, "HTTP请求完成");
        }
        response
            .headers_mut()
            .insert("x-request-id", request_id.parse().expect("UUID header"));
        response
    }
    .instrument(span)
    .await
}
