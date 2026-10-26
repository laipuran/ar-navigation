# 对话接口（草案）

## POST /api/v1/assistant/messages

### 请求

`Content-Type: application/json`

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| conversation_id | string | 否 | 首轮省略；续聊使用上次成功响应返回的值 |
| message | string | 是 | 用户消息，不能为空 |

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
| 410 | CONVERSATION_UNAVAILABLE | 提示会话不可用，用户确认后开始新对话 |
| 429 | RATE_LIMITED | 按Retry-After响应头指定的秒数等待后原样重试 |
| 502 | MODEL_UNAVAILABLE | 模型服务失败；允许用户原样重试 |
| 504 | REQUEST_TIMEOUT | 本轮执行超时；允许用户原样重试 |
| 500 | INTERNAL_ERROR | 显示失败；允许用户原样重试 |
