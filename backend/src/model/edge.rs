use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// 无向连接：端点排列顺序不表示通行方向。
/// 两个端点必须存在且互不相同，由加载器校验图级约束。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub endpoints: [NodeId; 2],
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EdgeKind {
    Walk {
        /// 实际行走距离，单位为米；必须是大于零的有限数值。
        distance_m: f64,
    },
    StairFlight {
        /// 沿楼梯实际行走的距离，而非垂直高度；单位为米，必须是大于零的有限数值。
        distance_m: f64,
    },
}

impl EdgeKind {
    pub fn distance_m(&self) -> f64 {
        match self {
            Self::Walk { distance_m } | Self::StairFlight { distance_m } => *distance_m,
        }
    }
}
