use ar_navigation::model::{Edge, EdgeKind, FloorId, Node, NodeId, NodeKind, RoomId};

#[test]
fn represents_room_arrival_and_floor() {
    let node = Node {
        id: NodeId("node-305".into()),
        floor_id: FloorId("floor-3".into()),
        kind: NodeKind::Room {
            room_id: RoomId("room-305".into()),
        },
    };

    assert_eq!(node.floor_id, FloorId("floor-3".into()));
    assert_eq!(
        node.kind,
        NodeKind::Room {
            room_id: RoomId("room-305".into()),
        }
    );
}

#[test]
fn represents_gate_stairs_and_clockwise_junction() {
    let gate = NodeKind::Gate {
        description: "教学楼东门".into(),
    };
    assert!(matches!(gate, NodeKind::Gate { .. }));
    assert!(matches!(NodeKind::Stairs, NodeKind::Stairs));

    let neighbors = vec![
        NodeId("a".into()),
        NodeId("b".into()),
        NodeId("c".into()),
        NodeId("d".into()),
    ];
    let junction = NodeKind::Junction {
        neighbors_clockwise: neighbors.clone(),
    };

    let NodeKind::Junction {
        neighbors_clockwise,
    } = junction
    else {
        panic!("expected a junction");
    };
    assert_eq!(neighbors_clockwise, neighbors);
}

#[test]
fn represents_walk_and_stair_connections() {
    let walk = Edge {
        endpoints: [NodeId("room".into()), NodeId("junction".into())],
        kind: EdgeKind::Walk { distance_m: 6.0 },
    };
    let stairs = Edge {
        endpoints: [NodeId("landing-1".into()), NodeId("landing-2".into())],
        kind: EdgeKind::StairFlight { distance_m: 12.0 },
    };

    assert_eq!(walk.kind, EdgeKind::Walk { distance_m: 6.0 });
    assert_eq!(stairs.kind, EdgeKind::StairFlight { distance_m: 12.0 });
}
