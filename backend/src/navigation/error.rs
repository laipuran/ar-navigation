use std::path::PathBuf;

use thiserror::Error;

use crate::model::{BuildingId, NodeId};

#[derive(Debug, Error)]
pub enum NavigationError {
    #[error("非法输入：{0}")]
    InvalidInput(String),
    #[error("读取楼栋文件 {path} 失败：{source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("导航 JSON 格式错误：{0}")]
    Json(#[from] serde_json::Error),
    #[error("楼栋数据无效：{0}")]
    InvalidData(String),
    #[error("楼栋重复：{0:?}")]
    BuildingAlreadyLoaded(BuildingId),
    #[error("楼栋不存在：{0:?}")]
    BuildingNotFound(BuildingId),
    #[error("楼栋 {building_id:?} 中节点不存在：{node_id:?}")]
    NodeNotFound {
        building_id: BuildingId,
        node_id: NodeId,
    },
    #[error("楼栋 {building_id:?} 中 {from:?} 到 {to:?} 不连通")]
    NoRoute {
        building_id: BuildingId,
        from: NodeId,
        to: NodeId,
    },
    #[error("路线总距离超出 f64 有限值范围")]
    DistanceOverflow,
}
