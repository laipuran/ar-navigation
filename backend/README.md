# 后端导航库

Rust进程内业务库，不监听端口，不调用自身HTTP接口，不依赖LLM计算路线。

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

模型工具适配与预算/超时控制、对话会话和HTTP接口、跨楼栋规划、连续定位与AR。业务错误尚未绑定HTTP状态码。

## 检查

在仓库根目录执行：

```sh
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```
