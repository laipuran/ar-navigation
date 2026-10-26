//! 导航领域类型；不负责文件读取、图构造或算法执行。

pub mod building;
pub mod edge;
pub mod node;
pub mod route;

pub use building::{BuildingId, BuildingSummary};
pub use edge::{Edge, EdgeKind};
pub use node::{FloorId, Node, NodeId, NodeKind, NodeQuery, NodeType, RoomId};
pub use route::{JunctionDirection, NavigationStep, Route, RouteLeg};
