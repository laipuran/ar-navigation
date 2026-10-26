use super::{EdgeKind, FloorId, NodeId};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Route {
    pub distance_m: f64,
    pub path: Vec<NodeId>,
    /// 与 path 相邻节点一一对应，方向始终为实际行走方向。
    pub legs: Vec<RouteLeg>,
    pub steps: Vec<NavigationStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouteLeg {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JunctionDirection {
    ClockwiseExit {
        number: usize,
    },
    Return,
    /// 路口起步时没有来向，只指明下一个节点。
    UnknownApproach,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NavigationStep {
    Junction {
        at: NodeId,
        toward: NodeId,
        direction: JunctionDirection,
    },
    Walk {
        from: NodeId,
        to: NodeId,
        distance_m: f64,
    },
    StairFlight {
        from: NodeId,
        to: NodeId,
        from_floor: FloorId,
        to_floor: FloorId,
        distance_m: f64,
    },
    Arrive {
        at: NodeId,
    },
}
