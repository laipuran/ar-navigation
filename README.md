# AI校园室内精准导航与AR实景指引

从课表上的一个教室名，到真正走到教室门口。

本项目面向武汉大学校园找路场景，希望帮助不熟悉校园的同学弄清教学楼简称、找到合适的入口，并沿楼内走廊和楼梯到达目标教室，而不只是停在地图上的楼栋位置。

## 我们想做什么

- **看懂上课地点**：将课表里的简称、教室编号转成明确的楼栋和房间。
- **走到教室门口**：把教学楼入口、走廊、楼梯和教室串成一条可走的路线。
- **在现场看懂指引**：先提供清楚的路线说明，再探索用AR把指引叠加到实景中。

## 当前进展

项目处于早期开发阶段，后端已支持按JSON schema加载不同教学楼文件、校验拓扑、查询节点、计算单栋楼内最短路线并生成确定性指引。使用方式见[后端说明](backend/README.md)。

AI对话、室内定位和AR实景指引尚未实现。前端使用Vue 3、TypeScript和Vite；后端使用Rust，为进程内导航库，尚无可启动的API服务。

后端设计文档分为两部分，具体实现范围见后端说明：

- [后端内部接口 Spec](docs/specs/backend-internal.md)：进程内查询、算路和工具执行，不使用REST接口。
- [对外对话接口 Spec](docs/specs/api.md)：面向网页对话的接口草案，尚未实现。

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

使用支持Rust 2024 edition的工具链，在仓库根目录运行检查：

```sh
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```
