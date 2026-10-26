# AI校园室内精准导航与AR实景指引

面向“AI校园室内精准导航与AR实景指引”赛题的项目。

当前处于领域建模阶段：先讨论不含AR的室内导航，依次确定模型、数据schema和测量需求。此前方案文档已删除，未确认的设计不作为实施约定。

## 当前项目

Monorepo：`frontend/`为Vue 3 + TypeScript + Vite前端，`backend/`为Rust项目。前端暂保留官方模板页面；后端仍为无第三方依赖的模型库，不包含HTTP服务、数据库、序列化格式、图校验器或导航算法。

- [`frontend/src/`](frontend/src/)：Vue前端源码。
- [`backend/src/model.rs`](backend/src/model.rs)：节点、节点变体、双向边及边变体。
- [`backend/tests/model.rs`](backend/tests/model.rs)：模型构造与属性测试，不替代图约束验证。
- [`GLOSSARY.md`](GLOSSARY.md)：已确认的领域术语。

节点与楼层ID暂使用字符串新类型，不约定编码格式或楼层顺序。边默认双向；Junction按俯视顺时针记录邻居，来路不计入出口序号。

### 前端

推荐使用Node.js 24 LTS和npm。进入前端目录后执行：

```sh
cd frontend
npm install
npm run dev
```

类型检查及构建：

```sh
npm run typecheck
npm run build
```

### 后端

从仓库根目录执行（需要支持Rust 2024 edition的工具链）：

```sh
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

后端目前是库，尚无可启动的Web服务。

数据来源已确定为消防图，采用JSON整理拓扑与补测距离；具体schema尚待确认。
