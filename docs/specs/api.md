# 对话接口

Swagger UI：`/swagger-ui/`；OpenAPI：`/api-docs/openapi.json`。当前为非流式JSON接口。

所有HTTP响应包含服务端生成的`X-Request-ID`，用于关联后端日志，与会话ID无关，不影响请求重试语义。

## POST /api/v1/assistant/messages

### 请求

`Content-Type: application/json`

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| conversation_id | string | 否 | 首轮省略；续聊使用上次成功响应返回的值 |
| message | string | 是 | 用户消息，不得全为空白，最多4,000个Unicode字符 |

拒绝未知字段、非法JSON与超过32 KiB的请求体，统一返回`400 INVALID_REQUEST`。`conversation_id`最长128字节，不能是空字符串。

首轮：

```json
{
  "message": "怎么去305？"
}
```

### 成功响应：200

`Content-Type: application/json`

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| conversation_id | string | 首轮由后端生成并返回，后续保持不变 |
| reply | string | 可直接展示的回答或追问 |

```json
{
  "conversation_id": "c_7fbc6293c51c450ba0c6ab5fe0fb6395",
  "reply": "你现在在哪栋教学楼、哪个入口？"
}
```

续聊：

```json
{
  "conversation_id": "c_7fbc6293c51c450ba0c6ab5fe0fb6395",
  "message": "我在教四东门。"
}
```

### 会话规则

- 后端保存上下文；前端只发送本轮消息，不回传历史消息或工具结果。
- 同一会话的成功响应保持相同conversation_id。追问和告知无法到达也属于成功响应，后续可以继续对话。
- 首轮不传任何ID；后端创建会话，并在首轮成功响应中返回conversation_id。
- 开始新对话时省略conversation_id，后端返回新的会话ID。
- 每个会话同时只处理一条新消息；前端等待响应后才能发送下一条。
- 一轮成功后，用户消息与该轮完整上下文一起提交；系统失败不留下半轮对话。
- 本接口不保证重发请求去重。网络中断或未收到响应时，前端不自动重发；首轮重发可能创建新会话，续聊重发可能重复追加消息。
- 会话不存在或失效时返回CONVERSATION_UNAVAILABLE，不静默创建新会话。前端提示用户重新开始；用户确认后，按新对话规则提交。
- conversation_id由后端生成且不可猜测，作为会话访问凭据，不放进URL或日志。
- 当前会话只存于单进程内存；重启后失效。空闲TTL默认30分钟，由`SESSION_TTL_SECONDS`配置；每轮成功后刷新，失败不刷新。正在执行的会话不会被清理。
- 历史达到配置的大小上限时返回`CONVERSATION_LIMIT_REACHED`，不静默截断历史；提示用户开始新对话。

### 错误响应

```json
{
  "error": {
    "code": "CONVERSATION_UNAVAILABLE",
    "message": "会话不存在或已失效，请重新开始对话。"
  }
}
```

| HTTP状态 | code | 客户端处理 |
| --- | --- | --- |
| 400 | INVALID_REQUEST | 修正字段类型或空消息 |
| 409 | CONVERSATION_BUSY | 会话正在处理另一条消息；等待后原样重试 |
| 409 | CONVERSATION_LIMIT_REACHED | 提示上下文达到上限，用户确认后开始新对话 |
| 410 | CONVERSATION_UNAVAILABLE | 提示会话不可用，用户确认后开始新对话 |
| 429 | RATE_LIMITED | 按Retry-After响应头指定的秒数等待后原样重试 |
| 502 | MODEL_UNAVAILABLE | LLM服务失败、协议异常或调用预算耗尽；允许用户原样重试 |
| 504 | REQUEST_TIMEOUT | 本轮或单次LLM请求超时；允许用户原样重试 |
| 500 | INTERNAL_ERROR | 显示失败；允许用户原样重试 |
