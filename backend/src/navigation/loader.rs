//! 文件与 JSON 适配：产出已校验楼栋，查询模块不访问文件系统。

use std::path::Path;

use serde::Deserialize;

use super::{error::NavigationError, graph::Graph};
use crate::model::{BuildingSummary, Edge, Node};

/// 只能由加载器产生；持有已校验的不可变图，不承担搜索、算路或指引生成。
pub struct LoadedBuilding {
    pub(super) summary: BuildingSummary,
    pub(super) graph: Graph,
}

impl LoadedBuilding {
    pub fn summary(&self) -> &BuildingSummary {
        &self.summary
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingData {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

pub fn load_building_file(
    summary: BuildingSummary,
    path: impl AsRef<Path>,
) -> Result<LoadedBuilding, NavigationError> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(path).map_err(|source| NavigationError::Io {
        path: path.to_owned(),
        source,
    })?;
    load_building_json(summary, &json)
}

pub fn load_building_json(
    summary: BuildingSummary,
    json: &str,
) -> Result<LoadedBuilding, NavigationError> {
    if summary.id.0.trim().is_empty() || summary.name.trim().is_empty() {
        return Err(NavigationError::InvalidInput(
            "楼栋 ID 与名称不能为空".into(),
        ));
    }
    let data: BuildingData = serde_json::from_str(json)?;
    let graph = Graph::new(data.nodes, data.edges)?;
    Ok(LoadedBuilding { summary, graph })
}
