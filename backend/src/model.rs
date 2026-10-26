//! The agreed indoor navigation model, without AR or storage concerns.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub floor_id: FloorId,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// The arrival point outside the room's marked front door, not its interior.
    Room {
        room_id: RoomId,
    },
    Gate {
        description: String,
    },
    /// A stair landing or access point, not an entire multi-floor staircase.
    Stairs,
    Junction {
        /// Adjacent nodes in clockwise order, viewed from above the floor.
        ///
        /// The first item is arbitrary. Each neighbor must appear exactly once,
        /// and the list must match the connections defined by edges. Each pair
        /// of nodes has at most one edge, so a neighbor identifies a connection.
        /// For instructions, count clockwise after the incoming neighbor,
        /// excluding that incoming connection. Returning along it is a U-turn.
        neighbors_clockwise: Vec<NodeId>,
    },
}

/// An undirected connection: endpoint order does not specify travel direction.
///
/// Both endpoints must exist and be distinct. These graph-level constraints
/// are documented here; validation will be designed separately.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub endpoints: [NodeId; 2],
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeKind {
    Walk {
        /// Actual walking distance in meters; finite and greater than zero.
        distance_m: f64,
    },
    StairFlight {
        /// Distance along the stairs, not vertical height; finite and positive.
        distance_m: f64,
    },
}
