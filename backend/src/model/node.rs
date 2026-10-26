use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FloorId(pub i32);

impl<'de> Deserialize<'de> for FloorId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // JSON Schema 的 integer 按数值判定，1.0、1e0 也表示整数。
        let number = serde_json::Number::deserialize(deserializer)?;
        let value = number
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("楼层必须是整数"))?;
        if value.fract() != 0.0 || value < i32::MIN as f64 || value > i32::MAX as f64 {
            return Err(serde::de::Error::custom("楼层必须是 i32 范围内的整数"));
        }
        Ok(Self(value as i32))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: NodeId,
    pub floor_id: FloorId,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeKind {
    /// 房间有标示的前门外的到达点，不表示房间内部。
    Room {
        name: String,
    },
    Gate {
        description: String,
    },
    /// 楼梯平台或出入口，不表示整座跨层楼梯。
    Stairs {},
    Junction {
        /// 俯视顺时针排列的实际邻居，仅用于指引，不建立连接。
        /// 首项任意；以进入通道为起点计数，来路不计入出口序号。
        neighbors: Vec<NodeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Room,
    Gate,
    Stairs,
    Junction,
}

impl NodeType {
    pub(crate) fn of(kind: &NodeKind) -> Self {
        match kind {
            NodeKind::Room { .. } => Self::Room,
            NodeKind::Gate { .. } => Self::Gate,
            NodeKind::Stairs {} => Self::Stairs,
            NodeKind::Junction { .. } => Self::Junction,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct NodeQuery {
    /// 大小写敏感的字面子串；不做分词、编号解析或名称猜测。
    pub text: Option<String>,
    pub kind: Option<NodeType>,
    pub floor_id: Option<FloorId>,
}
