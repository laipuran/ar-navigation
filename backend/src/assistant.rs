//! LLM-backed conversations with atomic turns and expiring in-memory history.

mod llm;
mod tools;
pub use llm::{Llm, LlmConfig};

use crate::navigation::Navigation;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, Semaphore};
use tokio::time::Instant;
use tracing::Instrument;
use utoipa::ToSchema;

pub struct AssistantConfig {
    pub session_ttl: Duration,
    pub turn_timeout: Duration,
    pub max_sessions: usize,
    pub max_concurrent_turns: usize,
    pub max_llm_calls: usize,
    pub max_tool_calls: usize,
    pub max_history_bytes: usize,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            session_ttl: Duration::from_secs(30 * 60),
            turn_timeout: Duration::from_secs(60),
            max_sessions: 1000,
            max_concurrent_turns: 16,
            max_llm_calls: 8,
            max_tool_calls: 12,
            max_history_bytes: 256 * 1024,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MessageRequest {
    /// 首轮省略；续聊使用上次成功响应的值，不发送历史消息。
    #[schema(max_length = 128)]
    pub conversation_id: Option<String>,
    /// 用户本轮消息，不能全为空白，最多 4,000 个 Unicode 字符。
    #[schema(
        min_length = 1,
        max_length = 4000,
        example = "我在演示楼东门，怎么去201？"
    )]
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub conversation_id: String,
    pub reply: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AssistantError {
    #[error("消息不能为空或超过长度限制，请检查请求字段。")]
    InvalidRequest,
    #[error("LLM 服务暂不可用，请重试。")]
    ModelUnavailable,
    #[error("会话不存在或已失效，请重新开始对话。")]
    ConversationUnavailable,
    #[error("会话正在处理另一条消息，请稍后重试。")]
    ConversationBusy,
    #[error("服务容量已满，请稍后重试。")]
    RateLimited,
    #[error("本轮执行超时，请重试。")]
    RequestTimeout,
    #[error("会话上下文已达上限，请开始新对话。")]
    ConversationLimitReached,
    #[error("服务内部错误，请重试。")]
    InternalError,
}

impl AssistantError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::ModelUnavailable => "MODEL_UNAVAILABLE",
            Self::ConversationUnavailable => "CONVERSATION_UNAVAILABLE",
            Self::ConversationBusy => "CONVERSATION_BUSY",
            Self::RateLimited => "RATE_LIMITED",
            Self::RequestTimeout => "REQUEST_TIMEOUT",
            Self::ConversationLimitReached => "CONVERSATION_LIMIT_REACHED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Clone)]
pub struct Assistant {
    inner: Arc<Inner>,
}

struct Inner {
    navigation: Arc<Navigation>,
    llm: Llm,
    config: AssistantConfig,
    turns: Semaphore,
    tool_slots: Arc<Semaphore>,
    sessions: Mutex<HashMap<String, Arc<AsyncMutex<Session>>>>,
}

struct Session {
    history: Vec<Value>,
    last_success: Instant,
}

// A failed/cancelled first turn removes its unexposed reservation. Existing
// sessions retain committed history; the owned guard always releases busy state.
struct Turn {
    inner: Arc<Inner>,
    id: String,
    session: OwnedMutexGuard<Session>,
    unpublished: bool,
}

impl Drop for Turn {
    fn drop(&mut self) {
        if self.unpublished {
            tracing::debug!("丢弃未提交的新会话");
            self.inner
                .sessions
                .lock()
                .expect("session table poisoned")
                .remove(&self.id);
        }
    }
}

impl Assistant {
    pub fn new(navigation: Navigation, llm: Llm, config: AssistantConfig) -> Result<Self, String> {
        if config.session_ttl.is_zero()
            || config.turn_timeout.is_zero()
            || config.max_sessions == 0
            || config.max_concurrent_turns == 0
            || config.max_concurrent_turns > Semaphore::MAX_PERMITS
            || config.max_llm_calls == 0
            || config.max_tool_calls == 0
            || config.max_history_bytes < 1024
        {
            return Err("Invalid assistant limits".into());
        }
        Ok(Self {
            inner: Arc::new(Inner {
                navigation: Arc::new(navigation),
                llm,
                turns: Semaphore::new(config.max_concurrent_turns),
                tool_slots: Arc::new(Semaphore::new(config.max_concurrent_turns)),
                config,
                sessions: Mutex::new(HashMap::new()),
            }),
        })
    }

    #[tracing::instrument(name = "assistant.turn", skip_all)]
    pub async fn send(&self, request: MessageRequest) -> Result<MessageResponse, AssistantError> {
        if request.message.trim().is_empty()
            || request.message.chars().count() > 4000
            || request
                .conversation_id
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 128)
        {
            return Err(AssistantError::InvalidRequest);
        }
        let mut turn = self.begin(request.conversation_id)?;
        let _permit = self
            .inner
            .turns
            .try_acquire()
            .map_err(|_| AssistantError::RateLimited)?;
        let mut history = turn.session.history.clone();
        history.push(json!({"role":"user","content":request.message}));
        let reply = tokio::time::timeout(self.inner.config.turn_timeout, self.run(&mut history))
            .await
            .map_err(|_| AssistantError::RequestTimeout)??;
        turn.session.history = history;
        turn.session.last_success = Instant::now();
        turn.unpublished = false;
        tracing::debug!("对话轮次已提交");
        Ok(MessageResponse {
            conversation_id: turn.id.clone(),
            reply,
        })
    }

    /// Reclaim idle sessions without touching turns in flight.
    pub fn cleanup_expired(&self) {
        let mut sessions = self.inner.sessions.lock().expect("session table poisoned");
        let before = sessions.len();
        sessions.retain(|_, session| {
            session.try_lock().map_or(true, |s| {
                s.last_success.elapsed() < self.inner.config.session_ttl
            })
        });
        let removed = before - sessions.len();
        let remaining = sessions.len();
        drop(sessions);
        if removed > 0 {
            tracing::info!(removed, remaining, "已清理过期会话");
        }
    }

    fn begin(&self, id: Option<String>) -> Result<Turn, AssistantError> {
        self.cleanup_expired();
        let mut sessions = self.inner.sessions.lock().expect("session table poisoned");
        if let Some(id) = id {
            let session = sessions
                .get(&id)
                .ok_or(AssistantError::ConversationUnavailable)?
                .clone()
                .try_lock_owned()
                .map_err(|_| AssistantError::ConversationBusy)?;
            if session.last_success.elapsed() >= self.inner.config.session_ttl {
                sessions.remove(&id);
                return Err(AssistantError::ConversationUnavailable);
            }
            Ok(Turn {
                inner: self.inner.clone(),
                id,
                session,
                unpublished: false,
            })
        } else {
            if sessions.len() >= self.inner.config.max_sessions {
                return Err(AssistantError::RateLimited);
            }
            let id = uuid::Uuid::new_v4().simple().to_string();
            let session = Arc::new(AsyncMutex::new(Session {
                history: vec![json!({"role":"system","content":SYSTEM_PROMPT})],
                last_success: Instant::now(),
            }));
            sessions.insert(id.clone(), session.clone());
            tracing::debug!("已预留新会话");
            Ok(Turn {
                inner: self.inner.clone(),
                id,
                session: session.try_lock_owned().expect("new session"),
                unpublished: true,
            })
        }
    }

    async fn run(&self, history: &mut Vec<Value>) -> Result<String, AssistantError> {
        let mut tool_count = 0;
        let mut call_ids = HashSet::new();
        for index in 0..self.inner.config.max_llm_calls {
            self.check_size(history)?;
            let message = self
                .inner
                .llm
                .complete(history, tools::definitions())
                .instrument(tracing::debug_span!("llm.call", call_number = index + 1))
                .await?;
            if message.to_string().len() > self.inner.config.max_history_bytes {
                return Err(AssistantError::ModelUnavailable);
            }
            let calls = match message.get("tool_calls") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(calls)) => calls.clone(),
                _ => return Err(AssistantError::ModelUnavailable),
            };
            if calls.is_empty() {
                let reply = message["content"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or(AssistantError::ModelUnavailable)?
                    .to_owned();
                history.push(message);
                self.check_size(history)?;
                return Ok(reply);
            }
            tool_count += calls.len();
            if tool_count > self.inner.config.max_tool_calls {
                tracing::warn!(tool_count, "工具调用预算耗尽");
                return Err(AssistantError::ModelUnavailable);
            }
            // Validate the entire batch before executing any of it.
            for call in &calls {
                let id = call["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or(AssistantError::ModelUnavailable)?;
                if call["type"] != "function"
                    || !call["function"]["name"].is_string()
                    || !call["function"]["arguments"].is_string()
                    || !call_ids.insert(id.to_owned())
                {
                    return Err(AssistantError::ModelUnavailable);
                }
            }
            history.push(message);
            for call in calls {
                let navigation = self.inner.navigation.clone();
                let name = call["function"]["name"]
                    .as_str()
                    .expect("validated name")
                    .to_owned();
                let arguments = call["function"]["arguments"]
                    .as_str()
                    .expect("validated arguments")
                    .to_owned();
                // 工具名来自模型，仅记录白名单名称，避免将任意输入写入日志。
                let safe_name = match name.as_str() {
                    "list_buildings" | "search_nodes" | "get_node" | "find_route" => name.as_str(),
                    _ => "unknown",
                };
                let span = tracing::debug_span!("navigation.tool", tool = safe_name);
                let dispatch = tracing::dispatcher::get_default(Clone::clone);
                let permit = self
                    .inner
                    .tool_slots
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| AssistantError::ModelUnavailable)?;
                // CPU work must not block the async deadline. A cancelled turn may
                // leave read-only work running, but that work retains its slot.
                let result = tokio::task::spawn_blocking(move || {
                    // 显式传递span，使后台算路日志仍关联当前HTTP请求。
                    tracing::dispatcher::with_default(&dispatch, || {
                        span.in_scope(|| {
                            let _permit = permit;
                            let started = std::time::Instant::now();
                            let result = tools::execute(&navigation, &name, &arguments);
                            let elapsed_ms = started.elapsed().as_millis() as u64;
                            if result["ok"] == true {
                                tracing::debug!(elapsed_ms, "导航工具执行完成");
                            } else {
                                let error_code = result["error"]["code"]
                                    .as_str()
                                    .unwrap_or("NAVIGATION_ERROR");
                                tracing::warn!(elapsed_ms, error_code, "导航工具返回业务错误");
                            }
                            result
                        })
                    })
                })
                .await
                .map_err(|_| AssistantError::InternalError)?;
                history.push(
                    json!({"role":"tool","tool_call_id":call["id"],"content":result.to_string()}),
                );
            }
        }
        tracing::warn!("LLM请求次数预算耗尽");
        Err(AssistantError::ModelUnavailable)
    }

    fn check_size(&self, history: &[Value]) -> Result<(), AssistantError> {
        if serde_json::to_vec(history).expect("JSON values").len()
            > self.inner.config.max_history_bytes
        {
            Err(AssistantError::ConversationLimitReached)
        } else {
            Ok(())
        }
    }
}

const SYSTEM_PROMPT: &str = r#"你是校园室内寻路助手。通过查询、算路和可复核的推导帮助用户找到房间前门外。回答简洁，优先给出结论和下一步。

按以下流程处理每轮请求：

1. 明确任务。结合上下文整理楼栋、用户确认的当前起点和目的地。继续使用仍然有效的确认信息；用户移动或修改目的地后，更新对应信息。缺少信息时询问具体缺项，可用已查询到的节点作为选项。

2. 将描述对应到节点。用list_buildings确认楼栋，用search_nodes按名称、类型、楼层查找。一个候选时将用户描述对应到该节点；多个候选时展示名称、楼层等区别，请用户选择；空结果时说明本次楼栋与筛选条件，询问完整名称或调整查询。名称和编号用于检索，楼层取节点的floor_id。

3. 执行与任务对应的查询。算路使用已明确的楼栋、起点ID和终点ID调用find_route。保留这组选择与对应结果的关联；get_node用于补充节点信息，search_nodes用于查询候选，各自的成功与失败分别解释。

4. 从工具结果推导回答：
   - 入口数量：查询目标楼栋kind=gate的完整列表，按返回节点数计数；列出名称，让用户可以核对。带文本或楼层筛选时，数量的适用范围就是该筛选范围。
   - 成功路线：以path确定先后顺序，以legs和steps给出每段距离与行动，以distance_m给出总长；可用分段距离求和复核总长。将节点ID对应到查到的名称，Room的到达位置是房间前门外。
   - 路口指引：以进入节点、当前路口、离开节点对应的步骤为依据，将ClockwiseExit的number表达为“从来路起顺时针第N个出口”。描述左右或方位需要额外的朝向依据；UnknownApproach时以已知下一节点引导。
   - 楼梯：用from_floor、to_floor描述两端楼层。上楼或下楼的判断需要楼层高低语义；只有整数标识时直接描述从哪层到哪层。
   - NO_ROUTE：对应的起终点在当前图数据中不连通，因此本次无法提供路线。若寻找替代起点或入口，先查询候选，再对候选算路，以成功结果支持替代方案。

5. 解释与推断。把直接查询到的事实、由事实计算得到的结论、仍需验证的假设区分清楚。提出原因假设时，简要给出支持它的观测和可执行的验证办法；已有工具不足以检验时说明需要补充什么信息。节点名称用于辨认位置，通行状态需要相应的状态信息。优先回答用户实际需要的内容。

6. 回答前复核。核对当前楼栋与起终点、候选数量、距离和步骤是否对应同一任务。对关键结论给出足够核对的依据：入口数量对应完整gate列表的节点数；总距离对应所选路线各段distance_m之和，并与路线总长核对。直接使用本次查询值，无需展开内部思考过程。数据标记为演示时，说明路线使用合成数据，不代表真实校园。"#;
