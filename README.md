# AI校园室内精准导航与AR实景指引

从课表上的一个教室名，到真正走到教室门口。

本项目面向武汉大学校园找路场景，希望帮助不熟悉校园的同学弄清教学楼简称、找到合适的入口，并沿楼内走廊和楼梯到达目标教室，而不只是停在地图上的楼栋位置。

## 我们想做什么

- **看懂上课地点**：将课表里的简称、教室编号转成明确的楼栋和房间。
- **走到教室门口**：把教学楼入口、走廊、楼梯和教室串成一条可走的路线。
- **在现场看懂指引**：先提供清楚的路线说明，再探索用AR把指引叠加到实景中。

## 当前进展

项目处于早期开发阶段，后端已支持按JSON schema加载不同教学楼文件、校验拓扑、查询节点、计算单栋楼内最短路线并生成确定性指引。使用方式见[后端说明](backend/README.md)。

后端提供基于LLM工具调用的多轮寻路对话、带TTL的内存会话，以及可通过Swagger调试的HTTP接口。前端使用Vue 3、TypeScript和Vite，目前仍为模板页面；室内定位和AR实景指引尚未实现。后端使用Rust，导航计算保持为进程内业务库。

后端设计文档分为两部分，具体实现范围见后端说明：

- [后端内部接口 Spec](docs/specs/backend-internal.md)：进程内查询、算路和工具执行，不使用REST接口。
- [对外对话接口 Spec](docs/specs/api.md)：面向网页对话的HTTP契约。

## 本地开发

### 前端

建议使用Node.js 24 LTS和npm：

```sh
cd frontend
npm install
npm run dev
```

检查类型并构建：

```sh
npm run typecheck
npm run build
```

### 后端

配置LLM并启动HTTP与Swagger，见[后端说明](backend/README.md#启动与swagger调试)。

使用支持Rust 2024 edition的工具链，在仓库根目录运行检查：

```sh
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```
