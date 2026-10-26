# 后端

Rust导航库与HTTP对话入口。导航库在进程内查询和算路，不调用自身HTTP接口，不依赖LLM计算路线；对话层通过OpenAI-compatible协议接入可配置的LLM。

## 启动与Swagger调试

在仓库根目录复制配置模板，编辑`.env`填写`LLM_BASE_URL`、`LLM_API_KEY`和`LLM_MODEL`：

```sh
cp .env.example .env
```

配置完成后启动：

```sh
cargo run --manifest-path backend/Cargo.toml
```

浏览器打开 **http://127.0.0.1:3000/swagger-ui/**；OpenAPI位于`/api-docs/openapi.json`。Swagger资源打包在程序中，不依赖外部CDN。示例清单仅加载合成测试楼栋，不代表真实校园。先发送`{"message":"我在演示楼东门，怎么去201？"}`；续聊时带上成功响应中的`conversation_id`，不要发送历史消息。

程序从当前目录向上查找并加载首个`.env`，已有环境变量优先；找不到`.env`时仅使用环境变量，不自动加载`.env.example`。`.env`中的语法或读取错误会阻止启动，且不会打印文件内容。`.env`已被Git忽略。

`NAVIGATION_BUILDINGS_FILE`相对于加载到的`.env`所在目录解析；没有`.env`时相对于工作目录解析，也支持绝对路径。清单是JSON数组，元素为`id`、`name`、`path`；其中楼栋文件路径相对于清单所在目录解析。启动时整批加载并校验，任何楼栋失败则不启动HTTP。

| 环境变量 | 默认值 | 用途 |
| --- | --- | --- |
| `LLM_BASE_URL`、`LLM_API_KEY`、`LLM_MODEL` | 必填 | Chat Completions服务地址、凭据和模型标识 |
| `NAVIGATION_BUILDINGS_FILE` | 必填 | 楼栋清单文件 |
| `LLM_REQUEST_OPTIONS` | `{}` | 供应商需要的附加JSON对象选项；不能覆盖`model/messages/tools/stream/n/tool_choice` |
| `HTTP_BIND` | `127.0.0.1:3000` | HTTP监听地址 |
| `RUST_LOG` | `warn,ar_navigation=info` | tracing日志过滤，启动时在加载`.env`后读取 |
| `SESSION_TTL_SECONDS` | `1800` | 成功一轮后刷新的空闲TTL |
| `TURN_TIMEOUT_SECONDS` | `60` | 一轮总超时；单次LLM请求另有20秒上限 |
| `MAX_SESSIONS` | `1000` | 含正在创建的会话的容量 |
| `MAX_CONCURRENT_TURNS` | `16` | 并发轮次与后台导航任务上限 |
| `MAX_LLM_CALLS` | `8` | 每轮LLM请求次数，不自动重试 |
| `MAX_TOOL_CALLS` | `12` | 每轮工具调用数，非法调用同样计数 |
| `MAX_HISTORY_BYTES` | `262144` | 每会话完整JSON历史大小上限 |

供应商特有的模式开关通过`LLM_REQUEST_OPTIONS`配置，代码不绑定模型名称；实际支持的工具调用和扩展字段续接规则需要按所选服务验证。历史保留完整assistant消息，包括协议扩展字段，但不向前端返回这些内部字段。

会话仅在单进程内存中保存，重启失效，不支持多实例共享。成功时原子提交整轮历史；失败或执行取消不提交。正在执行的会话不会被TTL清理，空闲会话每分钟回收，并在请求时检查过期。同步导航在有并发上限的后台任务执行；超时后已开始的只读计算可能继续完成，但不会提交对话。

消息最多4,000个Unicode字符，请求体最多32 KiB；工具结果最多64 KiB，超限返回结构化错误，不静默截掉候选。历史超限返回`CONVERSATION_LIMIT_REACHED`，由用户开始新对话。完整契约见[对话接口](../docs/specs/api.md)。

这是本地调试入口，未提供账号认证或公网防护；不要直接暴露到公网。会话ID相当于访问凭据，不放进URL、日志或截图。不要提交`.env`或将真实密钥写进`.env.example`。

## 日志

日志输出到标准错误，默认记录启动、楼栋数量、HTTP状态与耗时、失败分类和过期会话清理数量。每个HTTP响应都有服务端生成的`X-Request-ID`，可在日志中搜索同一个`request_id`。

需要查看LLM请求次数、逐次耗时、工具名称及轮次提交过程时，在`.env`中设置并重启：

```dotenv
RUST_LOG=warn,ar_navigation=debug
```

HTTP、LLM与后台导航工具通过span关联。应用不打印消息正文或原始上游错误，SDK自身的正文日志被过滤；LLM失败记录错误分类和可用的HTTP状态码。

## 加载不同教学楼

每个JSON文件遵循[`navigation.schema.json`](../schemas/navigation.schema.json)，只含`nodes`与`edges`。楼栋ID和展示名由调用方提供，不向JSON添加schema未定义的字段。

```rust,no_run
use ar_navigation::model::{
    BuildingId, BuildingSummary, NodeId, NodeQuery, NodeType,
};
use ar_navigation::navigation::Navigation;
use ar_navigation::navigation::loader::load_building_file;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut buildings = Vec::new();
    for (id, name, path) in [
        ("teaching-4", "第四教学楼", "data/teaching-4.json"),
        ("teaching-5", "第五教学楼", "data/teaching-5.json"),
    ] {
        buildings.push(load_building_file(
            BuildingSummary { id: BuildingId(id.into()), name: name.into() },
            path,
        )?);
    }
    // 只注入通过校验的楼栋，Navigation 本身不访问文件系统。
    let navigation = Navigation::new(buildings)?;

    let building = BuildingId("teaching-4".into());
    let candidates = navigation.search_nodes(&building, &NodeQuery {
        text: Some("305".into()),
        kind: Some(NodeType::Room),
        ..Default::default()
    })?;
    // 候选不唯一时应向用户追问，不能自动选第一个。
    if let [destination] = candidates.as_slice() {
        // 起点必须来自用户确认或其他可信输入，不能由教室编号推测。
        let route = navigation.find_route(
            &building, &NodeId("east-gate".into()), &destination.id,
        )?;
        println!("{}米，路径：{:?}", route.distance_m, route.path);
    }
    Ok(())
}
```

示例中的数据路径与节点ID为占位值；测试中的两份合成楼栋文件位于[`tests/fixtures/`](tests/fixtures/)，不代表真实校园数据。

## 模块职责与依赖

```text
model/          building、node、edge、route 领域类型
navigation/
  loader.rs     文件读取、JSON解析，产出 LoadedBuilding
  graph.rs      校验与不可变图构造，只读访问图数据
  search.rs     文本、类型、楼层筛选
  routing.rs    接收 &Graph，计算最短路径
  guidance.rs   接收 &Graph 和计算出的路径，生成指引
  error.rs      错误类型
navigation.rs   接收已加载楼栋，协调四个业务查询入口
assistant.rs    会话、TTL、并发和整轮提交
assistant/
  llm.rs        LLM协议适配，保留完整消息，禁用隐式重试
  tools.rs      工具白名单、参数校验与导航结果序列化
http.rs         HTTP契约与Swagger/OpenAPI
main.rs         配置、楼栋加载、启动与过期清理
```

调用方先使用加载器，再通过`Navigation::new(buildings)`注入已校验楼栋；空导航使用`Navigation::default()`。不再通过`Navigation`加载或修改楼栋。`LoadedBuilding`只保存元数据和图，不执行搜索或路线算法。图的存储私有，其他模块只能只读访问；不额外引入trait或独立进程。

## 已实现的约定

- 加载器校验并构建一次内存图，查询不再读文件；也支持`navigation::loader::load_building_json`加载字符串。
- 文件结构、必填字段、字段类型、未知字段和空白名称严格校验；额外校验节点唯一、边引用、自环、重复无向边、有限正距离、连接类型和路口邻居一致性。
- 加载失败不会产出楼栋或影响已有导航。节点ID仅在楼栋内唯一。`Navigation::new`拒绝重复楼栋ID，整次构造失败而非覆盖；如需启动时整批加载，可先将所有加载结果收集为`Result<Vec<_>, _>`，全部成功后再注入。
- 楼栋列表按楼栋ID排序，搜索结果按节点ID排序。搜索是大小写敏感的字面包含匹配，各筛选条件取交集；同名结果完整保留。省略文本可列出节点，显式空白文本报`InvalidInput`。
- 双向边按实际行走距离计算最短路，使用Dijkstra，复杂度`O(V² + E)`。同距离先处理节点ID字典序较小者，等长路径保留首次发现的前驱；不受文件节点、边或边端点排列影响。这不是“完整路径字典序最小”的承诺。
- `Route.path`是有序节点ID，`legs`记录对应连接及实际行走方向，`steps`记录路口选择、逐段步行/楼梯和到达。楼梯指引只给出两端楼层，不推断上下楼。
- 路口按来向计算顺时针出口序号，来路不计数；路口起步返回`UnknownApproach`并指明下一节点，不伪造方向。
- 起终点相同返回单节点、零距离和到达步骤；断连报`NoRoute`；不存在的楼栋和节点分别报错。所有可达路径距离均溢出时报告`DistanceOverflow`，不伪装为断连。
- schema允许空图；空图可加载并返回空搜索结果。`StairFlight`必须连接两个楼梯节点，不额外假设楼层编号大小关系或禁止同层楼梯段。

## 尚未实现

前端对话界面、持久化/多实例会话、跨楼栋规划、连续定位与AR。导航业务错误作为结构化工具结果返回，不直接映射成HTTP错误。自然语言中的消歧与位置确认依赖LLM遵循指令，仍需真实模型验收，不等于具备连续定位。

## 检查

在仓库根目录执行：

```sh
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```
