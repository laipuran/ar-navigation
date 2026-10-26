use super::{error::NavigationError, graph::Graph};
use crate::model::{Node, NodeKind, NodeQuery, NodeType};

pub(super) fn search(graph: &Graph, query: &NodeQuery) -> Result<Vec<Node>, NavigationError> {
    if query
        .text
        .as_ref()
        .is_some_and(|text| text.trim().is_empty())
    {
        return Err(NavigationError::InvalidInput(
            "搜索文本不能为空，请省略文本以列出节点".into(),
        ));
    }
    Ok(graph
        .nodes()
        .iter()
        .filter(|node| {
            query
                .kind
                .is_none_or(|kind| kind == NodeType::of(&node.kind))
                && query
                    .floor_id
                    .as_ref()
                    .is_none_or(|floor| floor == &node.floor_id)
                && query.text.as_ref().is_none_or(|text| match &node.kind {
                    NodeKind::Room { name } => name.contains(text.as_str()),
                    NodeKind::Gate { description } => description.contains(text.as_str()),
                    _ => false,
                })
        })
        .cloned()
        .collect())
}
