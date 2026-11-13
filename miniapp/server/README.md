# miniapp server · 小程序后端（BFF）

为微信小程序提供地点解析、楼内算路、课表识别与对话转发服务。零依赖（仅 Node 内置模块），`node server.js` 即可启动。

## 运行

```sh
cd miniapp/server
node server.js                 # 默认端口 8787
PORT=9000 node server.js       # 自定义端口
node test.js                   # 自测（27 项断言）
```

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `PORT` | `8787` | 监听端口 |
| `ASSISTANT_BASE_URL` | `http://127.0.0.1:8080` | Rust 后端对话接口地址 |
| `TENCENT_OCR_SECRET_ID/KEY` | 无 | 配置后才能启用真实 OCR（当前恒为 mock） |

## 接口

所有响应携带 `X-Request-ID` 头；错误统一为 `{"error": {"code", "message"}}`，与 Rust 后端的错误结构一致。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/v1/health` | 健康检查 |
| GET | `/api/v1/buildings` | 楼栋列表（含是否已建室内图 `mapped`） |
| GET | `/api/v1/places/resolve?name=` | 黑话解析，如 `教四-302` |
| GET | `/api/v1/routes/plan?name=` | 门到房间的楼内路线 |
| POST | `/api/v1/ocr/recognize` | 课表识别（mock） |
| POST | `/api/v1/chat/messages` | 对话，转发至 Rust 后端 |

### GET /api/v1/places/resolve

```sh
curl "http://127.0.0.1:8787/api/v1/places/resolve?name=教四-302"
```

成功返回与小程序 `result` 页渲染字段一一对应（`titleText` / `floorText` / `entrance` 等），并附加：

- `hasIndoorMap`：该楼是否有室内导航图（与 Rust 后端共用数据）
- `roomNodeId`：房间在图中的节点 id，算路接口的输入

错误：`404 PLACE_NOT_FOUND`（含解析容错：空格、全角横线均可）。

### GET /api/v1/routes/plan

```sh
curl "http://127.0.0.1:8787/api/v1/routes/plan?name=教四-302"
```

从最优入口门到目标教室的最短路径（Dijkstra），返回：

```json
{
  "ok": true,
  "totalMeters": 50,
  "steps": [
    { "type": "walk",   "text": "沿走廊走 13 米，然后直行", "meters": 13 },
    { "type": "stairs", "text": "上楼梯到 2 楼", "meters": 12, "fromFloor": 1, "toFloor": 2 },
    { "type": "stairs", "text": "上楼梯到 3 楼", "meters": 12, "fromFloor": 2, "toFloor": 3 },
    { "type": "walk",   "text": "沿走廊走 13 米，然后左转", "meters": 13 },
    { "type": "arrive", "text": "到达 302", "meters": 0 }
  ]
}
```

转向由 `junction.neighbors` 的俯视顺时针顺序推断（右转/左转/直行）。错误码：`ROUTE_NOT_MAPPED`（该楼未建图）、`ROOM_NOT_IN_GRAPH`（图无此房间）、`ROUTE_UNREACHABLE`。

### POST /api/v1/chat/messages

请求/响应/错误码与 Rust 后端 `POST /api/v1/assistant/messages` 完全一致（见 `docs/specs/api.md`）。上游不可达时返回 `502 UPSTREAM_UNAVAILABLE`。

## 数据

`data/buildings.json` 是楼栋索引（元数据 + 别名 + 图文件指针），`data/*.json` 是单栋楼的图，格式严格遵循仓库根 `schemas/navigation.schema.json`，**与 Rust 后端共用同一份数据约定**。新增一栋楼：照 schema 写一份 `{id}.json`，在索引里登记即可，代码零改动。

## 设计边界

- 楼内算路在 BFF 独立实现（Dijkstra），因 Rust 后端的导航算路按 spec 为进程内、不开放 REST；后端未来开放算路接口后，只需替换 `lib/routing.js` 的 `planRoute`。
- OCR 当前恒为 mock（与小程序端 `services/ocr.js` 数据一致）；真实接入见 `lib/ocr.js` 注释。
- 生产部署要求 HTTPS + 已备案域名（小程序 request 合法域名要求）；开发阶段在开发者工具中勾选「不校验合法域名」。
