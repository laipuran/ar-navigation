//! 接收已校验楼栋的进程内导航入口；不读取文件，不实现图算法。

use std::collections::BTreeMap;

use crate::model::{Node, NodeId};

pub mod error;
mod graph;
mod guidance;
pub mod loader;
mod routing;
mod search;

pub use crate::model::{
    BuildingId, BuildingSummary, JunctionDirection, NavigationStep, NodeQuery, NodeType, Route,
    RouteLeg,
};
pub use error::NavigationError;
pub use loader::LoadedBuilding;

#[derive(Default)]
pub struct Navigation {
    buildings: BTreeMap<BuildingId, LoadedBuilding>,
}

impl Navigation {
    /// 注入完整通过校验的楼栋；重复 ID 拒绝整次构造，不覆盖已有楼栋。
    pub fn new(
        buildings: impl IntoIterator<Item = LoadedBuilding>,
    ) -> Result<Self, NavigationError> {
        let mut indexed = BTreeMap::new();
        for building in buildings {
            let id = building.summary.id.clone();
            if indexed.insert(id.clone(), building).is_some() {
                return Err(NavigationError::BuildingAlreadyLoaded(id));
            }
        }
        Ok(Self { buildings: indexed })
    }

    pub fn list_buildings(&self) -> Vec<BuildingSummary> {
        self.buildings.values().map(|b| b.summary.clone()).collect()
    }

    pub fn search_nodes(
        &self,
        building_id: &BuildingId,
        query: &NodeQuery,
    ) -> Result<Vec<Node>, NavigationError> {
        search::search(&self.building(building_id)?.graph, query)
    }

    pub fn get_node(
        &self,
        building_id: &BuildingId,
        node_id: &NodeId,
    ) -> Result<Node, NavigationError> {
        let building = self.building(building_id)?;
        Ok(building.graph.node(node_index(building, node_id)?).clone())
    }

    pub fn find_route(
        &self,
        building_id: &BuildingId,
        from: &NodeId,
        to: &NodeId,
    ) -> Result<Route, NavigationError> {
        let building = self.building(building_id)?;
        let graph = &building.graph;
        let (start, end) = (node_index(building, from)?, node_index(building, to)?);
        let path =
            routing::shortest_path(graph, start, end)?.ok_or_else(|| NavigationError::NoRoute {
                building_id: building_id.clone(),
                from: from.clone(),
                to: to.clone(),
            })?;
        let steps = guidance::generate_steps(graph, &path);
        Ok(Route {
            distance_m: path.distance_m,
            path: path
                .indices
                .iter()
                .map(|&i| graph.node(i).id.clone())
                .collect(),
            legs: path.legs,
            steps,
        })
    }

    fn building(&self, id: &BuildingId) -> Result<&LoadedBuilding, NavigationError> {
        self.buildings
            .get(id)
            .ok_or_else(|| NavigationError::BuildingNotFound(id.clone()))
    }
}

fn node_index(building: &LoadedBuilding, id: &NodeId) -> Result<usize, NavigationError> {
    building
        .graph
        .index(id)
        .ok_or_else(|| NavigationError::NodeNotFound {
            building_id: building.summary.id.clone(),
            node_id: id.clone(),
        })
}
