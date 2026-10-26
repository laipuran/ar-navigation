#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildingId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingSummary {
    pub id: BuildingId,
    pub name: String,
}
