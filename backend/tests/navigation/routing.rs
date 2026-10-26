use super::*;

#[test]
fn shortest_route_uses_real_distances_and_reverse_edges() {
    let nav = loaded();
    let forward = nav
        .find_route(&building("a"), &id("gate"), &id("upper"))
        .unwrap();
    assert_eq!(forward.distance_m, 14.0);
    assert_eq!(
        forward.path,
        ["gate", "junction", "stairs-1", "stairs-2", "upper"].map(id)
    );
    assert_eq!(forward.legs.len(), forward.path.len() - 1);
    assert!(forward.steps.contains(&NavigationStep::Junction {
        at: id("junction"),
        toward: id("stairs-1"),
        direction: JunctionDirection::ClockwiseExit { number: 2 },
    }));
    assert!(forward.steps.contains(&NavigationStep::StairFlight {
        from: id("stairs-1"),
        to: id("stairs-2"),
        from_floor: FloorId(1),
        to_floor: FloorId(2),
        distance_m: 6.0,
    }));
    let reverse = nav
        .find_route(&building("a"), &id("upper"), &id("gate"))
        .unwrap();
    assert_eq!(reverse.distance_m, forward.distance_m);
    assert_eq!(
        reverse.path,
        forward.path.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(reverse.steps.contains(&NavigationStep::StairFlight {
        from: id("stairs-2"),
        to: id("stairs-1"),
        from_floor: FloorId(2),
        to_floor: FloorId(1),
        distance_m: 6.0,
    }));
    assert_eq!(
        reverse.steps.last(),
        Some(&NavigationStep::Arrive { at: id("gate") })
    );
    for leg in reverse.legs {
        assert!(
            reverse
                .path
                .windows(2)
                .any(|p| p == [leg.from.clone(), leg.to.clone()])
        );
    }
}

#[test]
fn missing_building_missing_endpoints_and_disconnected_are_distinct() {
    let nav = loaded();
    assert!(matches!(
        nav.get_node(&building("missing"), &id("gate")),
        Err(NavigationError::BuildingNotFound(_))
    ));
    assert!(matches!(
        nav.find_route(&building("missing"), &id("gate"), &id("room")),
        Err(NavigationError::BuildingNotFound(_))
    ));
    for (from, to) in [
        ("missing", "room"),
        ("gate", "missing"),
        ("missing", "missing"),
    ] {
        assert!(
            matches!(nav.find_route(&building("a"), &id(from), &id(to)), Err(NavigationError::NodeNotFound { node_id, .. }) if node_id == id("missing"))
        );
    }
    assert!(matches!(
        nav.find_route(&building("a"), &id("gate"), &id("isolated")),
        Err(NavigationError::NoRoute { .. })
    ));
}

#[test]
fn equal_length_selection_is_stable_under_json_reordering() {
    let mut data = json!({
        "nodes": (["start", "end", "a", "b"].map(|name| json!({"id": name, "floor_id": 0, "kind": {"type": "room", "name": name}}))),
        "edges": ([["start", "b"], ["b", "end"], ["start", "a"], ["a", "end"]].map(|ends| json!({"endpoints": ends, "kind": {"type": "walk", "distance_m": 1}})))
    });
    let route = |data: &Value| {
        from_json(&data.to_string())
            .find_route(&building("a"), &id("start"), &id("end"))
            .unwrap()
    };
    let first = route(&data);
    assert_eq!(first.path, ["start", "a", "end"].map(id));
    data["nodes"].as_array_mut().unwrap().reverse();
    data["edges"].as_array_mut().unwrap().reverse();
    for edge in data["edges"].as_array_mut().unwrap() {
        edge["endpoints"].as_array_mut().unwrap().reverse();
    }
    assert_eq!(route(&data), first);
}

#[test]
fn overflowing_route_is_not_reported_as_disconnected() {
    let data = json!({
        "nodes": (["a", "b", "c"].map(|id| json!({"id": id, "floor_id": 1, "kind": {"type": "stairs"}}))),
        "edges": ([["a", "b"], ["b", "c"]].map(|ends| json!({"endpoints": ends, "kind": {"type": "walk", "distance_m": 1.7e308}})))
    });
    let nav = from_json(&data.to_string());
    assert!(matches!(
        nav.find_route(&building("a"), &id("a"), &id("c")),
        Err(NavigationError::DistanceOverflow)
    ));
    assert!(
        nav.find_route(&building("a"), &id("a"), &id("b"))
            .unwrap()
            .distance_m
            .is_finite()
    );
}
