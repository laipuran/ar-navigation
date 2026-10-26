use crate::{
    model::{BuildingId, FloorId, NodeId, NodeQuery, NodeType},
    navigation::{Navigation, NavigationError},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn definitions() -> Value {
    let string = json!({"type":"string"});
    let definitions = [
        (
            "list_buildings",
            "列出已加载楼栋的ID和名称。将用户提到的楼栋对应到返回项，再用其ID查询；有歧义时展示名称请用户确认。名称中的演示标识用于说明数据来源。",
            json!({}),
            vec![],
        ),
        (
            "search_nodes",
            "文本按字面包含匹配，类型和楼层条件取交集；省略文本可列出节点。按候选数量处理：一个则对应节点，多个则展示区别并追问，零个则说明查询条件并补充或调整检索。统计整栋楼入口时传building_id和kind=gate，省略text与floor_id，按完整返回列表计数并列出名称供核对。",
            json!({
                "building_id":string, "text":string, "kind":{"type":"string","enum":["room","gate","stairs","junction"]}, "floor_id":{"type":"integer"}
            }),
            vec!["building_id"],
        ),
        (
            "get_node",
            "根据楼栋和节点ID查询名称、描述、类型与楼层等元数据，用于辨认候选或解释路线节点。可达性通过find_route验证；元数据查询与算路各自对应不同结论。",
            json!({"building_id":string,"node_id":string}),
            vec!["building_id", "node_id"],
        ),
        (
            "find_route",
            "用已明确的楼栋、用户确认的当前起点和目的地ID计算最短路线。成功时按path和steps组织指引，用分段距离复核distance_m；路口number是相对来路的顺时针出口序号。NO_ROUTE表示这组起终点在当前图中不连通；探索替代方案时查询候选，再以新的find_route结果验证。解释原因时区分已有事实与需要额外观测才能检验的假设。",
            json!({"building_id":string,"from":string,"to":string}),
            vec!["building_id", "from", "to"],
        ),
    ];
    Value::Array(definitions.into_iter().map(|(name, description, properties, required)| json!({
        "type":"function", "function": {"name":name,"description":description,"parameters":{
            "type":"object", "properties":properties,"required":required,"additionalProperties":false
        }}
    })).collect())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    building_id: String,
    text: Option<String>,
    kind: Option<Kind>,
    floor_id: Option<i32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Room,
    Gate,
    Stairs,
    Junction,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    building_id: String,
    node_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Find {
    building_id: String,
    from: String,
    to: String,
}

pub(super) fn execute(navigation: &Navigation, name: &str, arguments: &str) -> Value {
    let result = match name {
        "list_buildings" => serde_json::from_str::<Empty>(arguments).map(|_| {
            Ok(json!(
                navigation
                    .list_buildings()
                    .into_iter()
                    .map(|b| json!({"id":b.id.0,"name":b.name}))
                    .collect::<Vec<_>>()
            ))
        }),
        "search_nodes" => serde_json::from_str::<Search>(arguments).map(|args| {
            navigation
                .search_nodes(
                    &BuildingId(args.building_id),
                    &NodeQuery {
                        text: args.text,
                        floor_id: args.floor_id.map(FloorId),
                        kind: args.kind.map(|kind| match kind {
                            Kind::Room => NodeType::Room,
                            Kind::Gate => NodeType::Gate,
                            Kind::Stairs => NodeType::Stairs,
                            Kind::Junction => NodeType::Junction,
                        }),
                    },
                )
                .map(|nodes| json!(nodes))
        }),
        "get_node" => serde_json::from_str::<Get>(arguments).map(|args| {
            navigation
                .get_node(&BuildingId(args.building_id), &NodeId(args.node_id))
                .map(|node| json!(node))
        }),
        "find_route" => serde_json::from_str::<Find>(arguments).map(|args| {
            navigation
                .find_route(
                    &BuildingId(args.building_id),
                    &NodeId(args.from),
                    &NodeId(args.to),
                )
                .map(|route| json!(route))
        }),
        _ => {
            return failure(
                "UNKNOWN_TOOL",
                "Only the declared navigation tools are allowed.",
            );
        }
    };
    let result = match result {
        Err(_) => failure(
            "INVALID_ARGUMENTS",
            "Arguments do not match the tool schema.",
        ),
        Ok(Err(error)) => {
            let code = match error {
                NavigationError::InvalidInput(_) => "INVALID_INPUT",
                NavigationError::BuildingNotFound(_) => "BUILDING_NOT_FOUND",
                NavigationError::NodeNotFound { .. } => "NODE_NOT_FOUND",
                NavigationError::NoRoute { .. } => "NO_ROUTE",
                NavigationError::DistanceOverflow => "DISTANCE_OVERFLOW",
                _ => "NAVIGATION_ERROR",
            };
            failure(code, &error.to_string())
        }
        Ok(Ok(data)) => json!({"ok":true,"data":data}),
    };
    if result.to_string().len() > 64 * 1024 {
        failure(
            "RESULT_TOO_LARGE",
            "Narrow the query or shorten invalid identifiers; no partial candidates are returned.",
        )
    } else {
        result
    }
}

fn failure(code: &str, message: &str) -> Value {
    json!({"ok":false,"error":{"code":code,"message":message}})
}
