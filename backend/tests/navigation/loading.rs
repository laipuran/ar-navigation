use super::*;

#[test]
fn loads_separate_files_and_scopes_ids_to_each_building() {
    assert!(Navigation::default().list_buildings().is_empty());
    let buildings = ["b", "a"].map(|key| {
        let path = format!(
            "{}/tests/fixtures/teaching-{key}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        load_building_file(summary(key), path).unwrap()
    });
    let nav = Navigation::new(buildings).unwrap();
    assert_eq!(nav.list_buildings(), vec![summary("a"), summary("b")]);
    assert_eq!(
        nav.get_node(&building("a"), &id("gate")).unwrap().floor_id,
        FloorId(1)
    );
    assert_eq!(
        nav.get_node(&building("b"), &id("gate")).unwrap().floor_id,
        FloorId(-1)
    );
    assert_eq!(
        nav.find_route(&building("a"), &id("gate"), &id("room"))
            .unwrap()
            .distance_m,
        5.0
    );
    assert_eq!(
        nav.find_route(&building("b"), &id("gate"), &id("room"))
            .unwrap()
            .distance_m,
        7.0
    );
    assert!(matches!(
        load_building_file(summary("missing"), "does-not-exist.json"),
        Err(NavigationError::Io { .. })
    ));
    assert_eq!(nav.list_buildings().len(), 2);
}

#[test]
fn injected_buildings_cannot_silently_overwrite_duplicate_ids() {
    let buildings = [
        load_building_json(summary("a"), A).unwrap(),
        load_building_json(summary("a"), include_str!("../fixtures/teaching-b.json")).unwrap(),
    ];
    assert!(
        matches!(Navigation::new(buildings), Err(NavigationError::BuildingAlreadyLoaded(id)) if id == building("a"))
    );
}

#[test]
fn queries_do_not_depend_on_the_lifetime_of_source_json() {
    let loaded = {
        let json = A.to_owned();
        load_building_json(summary("a"), &json).unwrap()
    };
    let nav = Navigation::new([loaded]).unwrap();
    assert_eq!(
        nav.find_route(&building("a"), &id("gate"), &id("room"))
            .unwrap()
            .distance_m,
        5.0
    );
}

#[test]
fn schema_integer_floor_accepts_integral_decimal_and_exponent_not_strings() {
    for representation in ["1", "1.0", "1e0", "-2147483648.0", "2147483647.0"] {
        let json = format!(
            r#"{{"nodes":[{{"id":"a","floor_id":{representation},"kind":{{"type":"stairs"}}}}],"edges":[]}}"#
        );
        let nav = from_json(&json);
        assert_eq!(
            nav.get_node(&building("a"), &id("a")).unwrap().floor_id.0 as f64,
            representation.parse::<f64>().unwrap()
        );
    }
    rejected(
        json!({"nodes": [{"id": "a", "floor_id": "1", "kind": {"type": "stairs"}}], "edges": []}),
    );
}

fn rejected(value: Value) {
    let nav = loaded();
    assert!(
        load_building_json(summary("bad"), &value.to_string()).is_err(),
        "accepted: {value}"
    );
    assert_eq!(nav.list_buildings(), vec![summary("a")]);
    assert!(matches!(
        nav.get_node(&building("bad"), &id("gate")),
        Err(NavigationError::BuildingNotFound(_))
    ));
    assert_eq!(
        nav.find_route(&building("a"), &id("gate"), &id("room"))
            .unwrap()
            .distance_m,
        5.0
    );
}

#[test]
fn rejects_invalid_schema_shapes_labels_and_distances() {
    let original: Value = serde_json::from_str(A).unwrap();
    let changes = [
        ("/extra", json!(true)),
        ("/nodes/0/extra", json!(true)),
        ("/nodes/0/kind/extra", json!(true)),
        ("/nodes/4/kind/extra", json!(true)),
        ("/nodes/0/id", json!(" \t")),
        ("/nodes/0/kind/description", json!(" ")),
        ("/nodes/2/kind/name", json!("")),
        ("/nodes/0/floor_id", json!(1.5)),
        ("/nodes/0/floor_id", json!(2147483648_i64)),
        ("/nodes/0/kind/type", json!("unknown")),
        ("/edges/0/extra", json!(true)),
        ("/edges/0/kind/extra", json!(true)),
        ("/edges/0/kind/distance_m", json!(0)),
        ("/edges/0/kind/distance_m", json!(-1)),
        ("/edges/0/kind/distance_m", Value::Null),
        ("/edges/0/endpoints", json!(["gate"])),
        ("/edges/0/endpoints", json!(["gate", "room", "upper"])),
    ];
    for (pointer, replacement) in changes {
        let mut data = original.clone();
        if let Some(slot) = data.pointer_mut(pointer) {
            *slot = replacement;
        } else {
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            data.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), replacement);
        }
        rejected(data);
    }
    rejected(json!({"nodes": []}));
    rejected(json!({"nodes": [], "edges": [], "unknown": 1}));
    for json in [
        "not json",
        "{\"nodes\":[],\"edges\":[]} garbage",
        "{\"nodes\":[],\"edges\":[{\"endpoints\":[\"a\",\"b\"],\"kind\":{\"type\":\"walk\",\"distance_m\":1e400}}]}",
    ] {
        assert!(load_building_json(summary("bad"), json).is_err());
    }
}

#[test]
fn rejects_invalid_graph_relations() {
    let original: Value = serde_json::from_str(A).unwrap();
    for (pointer, replacement) in [
        ("/nodes/2/id", json!("gate")),
        ("/edges/0/endpoints/0", json!("absent")),
        ("/edges/0/endpoints/1", json!("gate")),
        ("/edges/4/endpoints", json!(["junction", "gate"])),
        (
            "/nodes/1/kind/neighbors",
            json!(["gate", "room", "stairs-1"]),
        ),
        (
            "/nodes/1/kind/neighbors",
            json!(["gate", "room", "stairs-1", "room-copy", "absent"]),
        ),
        (
            "/nodes/1/kind/neighbors",
            json!(["gate", "room", "stairs-1", "room-copy", "gate"]),
        ),
        ("/nodes/0/floor_id", json!(3)),
        ("/edges/0/kind/type", json!("stair_flight")),
    ] {
        let mut data = original.clone();
        *data.pointer_mut(pointer).unwrap() = replacement;
        rejected(data);
    }
}

#[test]
fn building_metadata_must_be_nonblank_and_empty_graph_is_supported() {
    let empty = r#"{"nodes":[],"edges":[]}"#;
    for info in [
        BuildingSummary {
            id: building(" "),
            name: "楼栋".into(),
        },
        BuildingSummary {
            id: building("a"),
            name: "".into(),
        },
    ] {
        assert!(matches!(
            load_building_json(info, empty),
            Err(NavigationError::InvalidInput(_))
        ));
    }
    let nav = Navigation::new([load_building_json(summary("empty"), empty).unwrap()]).unwrap();
    assert!(
        nav.search_nodes(&building("empty"), &NodeQuery::default())
            .unwrap()
            .is_empty()
    );
}
