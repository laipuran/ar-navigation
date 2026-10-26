use ar_navigation::model::{FloorId, NodeId};
use ar_navigation::navigation::loader::{load_building_file, load_building_json};
use ar_navigation::navigation::{
    BuildingId, BuildingSummary, JunctionDirection, Navigation, NavigationError, NavigationStep,
    NodeQuery, NodeType,
};
use serde_json::{Value, json};

#[path = "navigation/guidance.rs"]
mod guidance;
#[path = "navigation/loading.rs"]
mod loading;
#[path = "navigation/routing.rs"]
mod routing;
#[path = "navigation/search.rs"]
mod search;

const A: &str = include_str!("fixtures/teaching-a.json");

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}
fn building(s: &str) -> BuildingId {
    BuildingId(s.into())
}
fn summary(s: &str) -> BuildingSummary {
    BuildingSummary {
        id: building(s),
        name: format!("教学楼{s}"),
    }
}
fn loaded() -> Navigation {
    from_json(A)
}
fn from_json(json: &str) -> Navigation {
    Navigation::new([load_building_json(summary("a"), json).unwrap()]).unwrap()
}
