//! 根据已计算路径生成确定性指引，不参与路径选择。

use super::{graph::Graph, routing::ComputedPath};
use crate::model::{EdgeKind, JunctionDirection, NavigationStep, NodeId, NodeKind};

pub(super) fn generate_steps(graph: &Graph, path: &ComputedPath) -> Vec<NavigationStep> {
    let mut steps = Vec::new();
    for (i, leg) in path.legs.iter().enumerate() {
        let node = graph.node(path.indices[i]);
        let next = graph.node(path.indices[i + 1]);
        if let NodeKind::Junction { neighbors } = &node.kind {
            let incoming = i.checked_sub(1).map(|p| &graph.node(path.indices[p]).id);
            steps.push(NavigationStep::Junction {
                at: node.id.clone(),
                toward: next.id.clone(),
                direction: junction_direction(neighbors, incoming, &next.id),
            });
        }
        steps.push(match leg.kind {
            EdgeKind::Walk { distance_m } => NavigationStep::Walk {
                from: leg.from.clone(),
                to: leg.to.clone(),
                distance_m,
            },
            EdgeKind::StairFlight { distance_m } => NavigationStep::StairFlight {
                from: leg.from.clone(),
                to: leg.to.clone(),
                from_floor: node.floor_id.clone(),
                to_floor: next.floor_id.clone(),
                distance_m,
            },
        });
    }
    let destination = graph.node(*path.indices.last().expect("computed path is nonempty"));
    steps.push(NavigationStep::Arrive {
        at: destination.id.clone(),
    });
    steps
}

fn junction_direction(
    neighbors: &[NodeId],
    incoming: Option<&NodeId>,
    next: &NodeId,
) -> JunctionDirection {
    let Some(incoming) = incoming else {
        return JunctionDirection::UnknownApproach;
    };
    if incoming == next {
        return JunctionDirection::Return;
    }
    let entry = neighbors
        .iter()
        .position(|n| n == incoming)
        .expect("validated incoming neighbor");
    let exit = neighbors
        .iter()
        .position(|n| n == next)
        .expect("validated outgoing neighbor");
    JunctionDirection::ClockwiseExit {
        number: (exit + neighbors.len() - entry) % neighbors.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junction_count_depends_on_approach_not_first_neighbor() {
        let ids: Vec<_> = ["a", "b", "c", "d"].map(|s| NodeId(s.into())).into();
        for rotation in 0..4 {
            let mut neighbors = ids.clone();
            neighbors.rotate_left(rotation);
            assert_eq!(
                junction_direction(&neighbors, Some(&ids[0]), &ids[2]),
                JunctionDirection::ClockwiseExit { number: 2 }
            );
            assert_eq!(
                junction_direction(&neighbors, Some(&ids[3]), &ids[2]),
                JunctionDirection::ClockwiseExit { number: 3 }
            );
            assert_eq!(
                junction_direction(&neighbors, Some(&ids[0]), &ids[0]),
                JunctionDirection::Return
            );
            assert_eq!(
                junction_direction(&neighbors, None, &ids[0]),
                JunctionDirection::UnknownApproach
            );
        }
    }
}
