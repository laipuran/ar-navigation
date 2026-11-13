# miniapp · 微信小程序端

校园室内导航的小程序前端，对应需求中的「看懂上课地点」「走到教室门口」「在现场看懂指引」三项能力。

## 如何打开

用**微信开发者工具**「导入项目」，目录选择本文件夹（`miniapp/`，不是仓库根目录）。

导入时会要求填写 AppID：

- 仓库里 `project.config.json` 的 appid 是**测试号**（`gh_` 开头），只对提交者本人有效
- 请换成你自己的：微信公众平台 → 管理 → 开发管理 → 开发设置 → AppID
- 没有正式 AppID 时可在导入时选择「测试号」

## 目录结构

```
miniapp/
├─ project.config.json      # 工程配置（miniprogramRoot 指向 miniprogram/）
├─ server/                  # 小程序后端（BFF，Node 零依赖）
│  ├─ server.js             # 入口：node server.js 启动，默认端口 8787
│  ├─ lib/                  # 地点解析 / Dijkstra 算路 / OCR 服务
│  ├─ data/                 # 楼栋图数据，格式遵循 schemas/navigation.schema.json
│  ├─ test.js               # 自测（27 项断言）
│  └─ README.md             # 接口文档
└─ miniprogram/             # 小程序代码根目录
   ├─ app.js / app.json / app.wxss
   ├─ data/
   │  ├─ places.js          # 地点"黑话"词典 + 解析函数
   │  └─ geo.js             # 楼栋经纬度（示例值，待测绘校正）
   ├─ services/
   │  └─ ocr.js             # OCR 服务层（当前为模拟实现）
   └─ pages/
      ├─ home/              # 首页：拍照识课表 / 手动查教室 / 历史记录
      ├─ result/            # 解析结果页：地点翻译 + 路线 + 耗时
      ├─ chat/              # AI 对话页：多轮追问
      ├─ timetable/         # 课表识别页：识别结果列表
      ├─ map/               # 地图页：校园定位 + 楼层示意
      └─ ar/                # AR 实景页：相机画面 + 方向箭头
```

六个页面在 `app.json` 的 `pages` 中注册，新增页面必须在这里登记，否则无法跳转。

## 当前实现状态

| 模块 | 状态 |
| --- | --- |
| 页面结构、全链路跳转 | 已完成 |
| 地点解析（简称 → 楼栋/房间/楼层） | 逻辑真实，词库仅 4 个示例地点 |
| 课表识别 | 流程真实，识别为模拟数据 |
| 地图与楼层示意 | 结构与样式真实，坐标为示例值 |
| AR 实景 | 相机叠加 + 箭头交互真实，无 3D 锚定 |
| AI 对话 | 多轮上下文真实，回复由本地规则生成 |

`data/places.js`、`data/geo.js`、`services/ocr.js` 三处是待替换的接入点：换成后端接口时页面代码无需改动。

## 后端服务（BFF）

`miniapp/server` 提供小程序专用接口：地点解析、楼内最短路径（Dijkstra）、课表识别、对话转发。

```sh
cd miniapp/server && node server.js    # 启动，端口 8787
node test.js                           # 自测
```

- 楼栋数据与 Rust 后端共用 `schemas/navigation.schema.json` 格式（单一数据源）
- 对话接口原样转发到 Rust 后端 `POST /api/v1/assistant/messages`，上游不可达返回 `502 UPSTREAM_UNAVAILABLE`
- 小程序端通过 `miniprogram/services/api.js` 访问，策略为**远程优先、本地兜底**：后端没启动时页面照常工作
- 开发者在工具「详情 → 本地设置」勾选「不校验合法域名」后即可联调；详见 `server/README.md`

## 已知限制

- **个人主体不支持 `web-view`**，因此无法内嵌 Cesium 三维场景，地图页改用原生 `map` 组件 + 自绘楼层示意图
- **`VKSession` 真 3D 锚定**机型兼容性差且模拟器不可用，AR 页改用 `<camera>` + 二维箭头叠加（与主流步行 AR 导航做法一致）
- `camera` 组件在 PC 模拟器中无法启动，AR 页需真机预览验证
