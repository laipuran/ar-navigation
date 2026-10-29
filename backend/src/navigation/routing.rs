//! 只计算路径，不生成指引，不读取楼栋元数据或文件。

use std::collections::VecDeque;

use super::{error::NavigationError, graph::Graph};
use crate::model::{EdgeKind, RouteLeg};

pub(super) struct ComputedPath {
    pub distance_m: f64,
    pub indices: Vec<usize>,
    pub legs: Vec<RouteLeg>,
}

/// 调用方先将已确认的节点 ID 解析为当前图中的索引；断连返回 None。
pub(super) fn shortest_path(
    graph: &Graph,
    start: usize,
    end: usize,
) -> Result<Option<ComputedPath>, NavigationError> {
    let n = graph.nodes().len();
    let mut distances = vec![f64::INFINITY; n];
    let mut visited = vec![false; n];
    let mut previous: Vec<Option<(usize, EdgeKind)>> = vec![None; n];
    distances[start] = 0.0;
    // O(V² + E) Dijkstra；同距离按 NodeId，等长时保留首次发现的前驱。
    loop {
        let current = (0..n)
            .filter(|&i| !visited[i] && distances[i].is_finite())
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]).then(a.cmp(&b)));
        let Some(current) = current else { break };
        if current == end {
            break;
        }
        visited[current] = true;
        for link in graph.links(current) {
            let candidate = distances[current] + link.kind.distance_m();
            if !visited[link.to] && candidate < distances[link.to] {
                distances[link.to] = candidate;
                previous[link.to] = Some((current, link.kind.clone()));
            }
        }
    }
    if !distances[end].is_finite() {
        return if is_reachable(graph, start, end) {
            Err(NavigationError::DistanceOverflow)
        } else {
            Ok(None)
        };
    }
    
    let mut indices = vec![end];
    let mut legs = Vec::new();
    let mut cursor = end;
    while cursor != start {
        let (prev, kind) = previous[cursor]
            .as_ref()
            .expect("reachable non-start has predecessor");
        legs.push(RouteLeg {
            from: graph.node(*prev).id.clone(),
            to: graph.node(cursor).id.clone(),
            kind: kind.clone(),
        });
        cursor = *prev;
        indices.push(cursor);
    }
    indices.reverse();
    legs.reverse();
    Ok(Some(ComputedPath {
        distance_m: distances[end],
        indices,
        legs,
    }))
}

fn is_reachable(graph: &Graph, start: usize, end: usize) -> bool {
    let mut reachable = vec![false; graph.nodes().len()];
    let mut queue = VecDeque::from([start]);
    reachable[start] = true;
    while let Some(i) = queue.pop_front() {
        if i == end {
            return true;
        }
        for link in graph.links(i) {
            if !reachable[link.to] {
                reachable[link.to] = true;
                queue.push_back(link.to);
            }
        }
    }
    false
}
