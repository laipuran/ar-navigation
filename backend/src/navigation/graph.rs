//! 校验并构建不可变图，连接关系只来自 Edge。

use std::collections::{BTreeMap, HashSet};

use super::error::NavigationError;
use crate::model::{Edge, EdgeKind, Node, NodeId, NodeKind};

pub(super) struct Link {
    pub to: usize,
    pub kind: EdgeKind,
}

pub(super) struct Graph {
    nodes: Vec<Node>,
    indices: BTreeMap<NodeId, usize>,
    links: Vec<Vec<Link>>,
}

impl Graph {
    pub fn new(mut nodes: Vec<Node>, edges: Vec<Edge>) -> Result<Self, NavigationError> {
        // 固定排序保证等长路线不受输入数组排列影响。
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        let indices = index_nodes(&nodes)?;
        let mut links: Vec<Vec<Link>> = (0..nodes.len()).map(|_| Vec::new()).collect();
        let mut pairs = HashSet::new();
        for edge in edges {
            let lookup = |id: &NodeId| {
                indices.get(id).copied().ok_or_else(|| {
                    NavigationError::InvalidData(format!("边引用了不存在的节点：{id:?}"))
                })
            };
            let [a, b] = &edge.endpoints;
            let (a, b) = (lookup(a)?, lookup(b)?);
            if a == b {
                return invalid("边两端必须不同");
            }
            if !pairs.insert((a.min(b), a.max(b))) {
                return invalid("重复的无向边");
            }
            validate_edge(&nodes[a], &nodes[b], &edge.kind)?;
            links[a].push(Link {
                to: b,
                kind: edge.kind.clone(),
            });
            links[b].push(Link {
                to: a,
                kind: edge.kind,
            });
        }
        for neighbors in &mut links {
            neighbors.sort_by_key(|link| link.to);
        }
        validate_junctions(&nodes, &links)?;
        Ok(Self {
            nodes,
            indices,
            links,
        })
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    pub fn index(&self, id: &NodeId) -> Option<usize> {
        self.indices.get(id).copied()
    }

    pub fn links(&self, index: usize) -> &[Link] {
        &self.links[index]
    }
}

fn index_nodes(nodes: &[Node]) -> Result<BTreeMap<NodeId, usize>, NavigationError> {
    let mut indices = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.id.0.trim().is_empty() {
            return invalid("节点 ID 不能为空");
        }
        if indices.insert(node.id.clone(), i).is_some() {
            return invalid(format!("节点 ID 重复：{:?}", node.id));
        }
        let label = match &node.kind {
            NodeKind::Room { name } => Some(name),
            NodeKind::Gate { description } => Some(description),
            _ => None,
        };
        if label.is_some_and(|s| s.trim().is_empty()) {
            return invalid(format!("节点 {:?} 的名称或描述不能为空", node.id));
        }
    }
    Ok(indices)
}

fn validate_edge(from: &Node, to: &Node, kind: &EdgeKind) -> Result<(), NavigationError> {
    let distance = kind.distance_m();
    if !distance.is_finite() || distance <= 0.0 {
        return invalid("边距离必须是有限正数");
    }
    match kind {
        EdgeKind::Walk { .. } if from.floor_id != to.floor_id => invalid("Walk 只能连接同层节点"),
        EdgeKind::StairFlight { .. }
            if !matches!(from.kind, NodeKind::Stairs {})
                || !matches!(to.kind, NodeKind::Stairs {}) =>
        {
            invalid("StairFlight 两端必须是楼梯节点")
        }
        _ => Ok(()),
    }
}

fn validate_junctions(nodes: &[Node], links: &[Vec<Link>]) -> Result<(), NavigationError> {
    for (i, node) in nodes.iter().enumerate() {
        if let NodeKind::Junction { neighbors } = &node.kind {
            let expected: HashSet<_> = links[i].iter().map(|l| &nodes[l.to].id).collect();
            let actual: HashSet<_> = neighbors.iter().collect();
            if actual.len() != neighbors.len() || actual != expected {
                return invalid(format!("路口 {:?} 的邻居与边不一致或有重复", node.id));
            }
        }
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, NavigationError> {
    Err(NavigationError::InvalidData(message.into()))
}
