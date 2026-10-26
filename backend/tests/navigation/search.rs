use super::*;

#[test]
fn search_intersects_filters_and_preserves_ambiguity() {
    let nav = loaded();
    let search = |query| nav.search_nodes(&building("a"), &query).unwrap();
    assert_eq!(
        search(NodeQuery {
            text: Some("101".into()),
            ..Default::default()
        })
        .len(),
        2
    );
    assert_eq!(
        search(NodeQuery {
            text: Some("东门".into()),
            kind: Some(NodeType::Gate),
            floor_id: Some(FloorId(1))
        })
        .len(),
        1
    );
    assert!(
        search(NodeQuery {
            text: Some("101".into()),
            floor_id: Some(FloorId(2)),
            ..Default::default()
        })
        .is_empty()
    );
    assert_eq!(
        search(NodeQuery {
            kind: Some(NodeType::Stairs),
            ..Default::default()
        })
        .len(),
        2
    );
    assert_eq!(
        search(NodeQuery {
            floor_id: Some(FloorId(2)),
            ..Default::default()
        })
        .len(),
        2
    );
    assert!(
        search(NodeQuery {
            text: Some("不存在".into()),
            ..Default::default()
        })
        .is_empty()
    );
    assert_eq!(search(NodeQuery::default()).len(), 8);
    assert!(matches!(
        nav.search_nodes(
            &building("a"),
            &NodeQuery {
                text: Some("  ".into()),
                ..Default::default()
            }
        ),
        Err(NavigationError::InvalidInput(_))
    ));
}
