use super::*;

#[test]
fn rotating_clockwise_neighbors_does_not_change_route_or_steps() {
    let nav = loaded();
    let expected = nav
        .find_route(&building("a"), &id("gate"), &id("upper"))
        .unwrap();
    let mut data: Value = serde_json::from_str(A).unwrap();
    for _ in 0..4 {
        data["nodes"][1]["kind"]["neighbors"]
            .as_array_mut()
            .unwrap()
            .rotate_left(1);
        let rotated = from_json(&data.to_string());
        assert_eq!(
            rotated
                .find_route(&building("a"), &id("gate"), &id("upper"))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn same_node_and_junction_start_do_not_invent_movement_or_approach() {
    let nav = loaded();
    let route = nav
        .find_route(&building("a"), &id("junction"), &id("junction"))
        .unwrap();
    assert_eq!(route.distance_m, 0.0);
    assert_eq!(route.path, vec![id("junction")]);
    assert!(route.legs.is_empty());
    assert_eq!(
        route.steps,
        vec![NavigationStep::Arrive { at: id("junction") }]
    );
    let route = nav
        .find_route(&building("a"), &id("junction"), &id("room"))
        .unwrap();
    assert_eq!(
        route.steps[0],
        NavigationStep::Junction {
            at: id("junction"),
            toward: id("room"),
            direction: JunctionDirection::UnknownApproach,
        }
    );
}
