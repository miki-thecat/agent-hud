use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A stable identity for a dependency node.
///
/// The scope is part of the identity so that, for example, task `42` in two
/// repositories cannot accidentally become the same graph node.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyNodeId {
    pub scope: String,
    pub value: String,
}

impl DependencyNodeId {
    pub fn new(scope: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyNode {
    pub id: DependencyNodeId,
    pub label: String,
    pub resolved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyGraphError {
    DuplicateNode(DependencyNodeId),
    UnknownNode(DependencyNodeId),
    DuplicateEdge {
        prerequisite: DependencyNodeId,
        dependent: DependencyNodeId,
    },
    SelfDependency(DependencyNodeId),
}

impl fmt::Display for DependencyGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(formatter, "duplicate dependency node {id:?}"),
            Self::UnknownNode(id) => write!(formatter, "unknown dependency node {id:?}"),
            Self::DuplicateEdge {
                prerequisite,
                dependent,
            } => write!(
                formatter,
                "duplicate dependency edge {prerequisite:?} -> {dependent:?}"
            ),
            Self::SelfDependency(id) => write!(formatter, "self dependency {id:?}"),
        }
    }
}

impl std::error::Error for DependencyGraphError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyGraph {
    nodes: BTreeMap<DependencyNodeId, DependencyNode>,
    prerequisites: BTreeMap<DependencyNodeId, BTreeSet<DependencyNodeId>>,
    dependents: BTreeMap<DependencyNodeId, BTreeSet<DependencyNodeId>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyGraphValidation {
    /// Strongly connected components containing cycles, in deterministic order.
    pub cycles: Vec<Vec<DependencyNodeId>>,
}

impl DependencyGraph {
    pub fn add_node(
        &mut self,
        id: DependencyNodeId,
        label: impl Into<String>,
        resolved: bool,
    ) -> Result<(), DependencyGraphError> {
        if self.nodes.contains_key(&id) {
            return Err(DependencyGraphError::DuplicateNode(id));
        }
        self.prerequisites.insert(id.clone(), BTreeSet::new());
        self.dependents.insert(id.clone(), BTreeSet::new());
        self.nodes.insert(
            id.clone(),
            DependencyNode {
                id,
                label: label.into(),
                resolved,
            },
        );
        Ok(())
    }

    pub fn remove_node(&mut self, id: &DependencyNodeId) -> Option<DependencyNode> {
        let node = self.nodes.remove(id)?;
        for prerequisite in self.prerequisites.remove(id).unwrap_or_default() {
            if let Some(dependents) = self.dependents.get_mut(&prerequisite) {
                dependents.remove(id);
            }
        }
        for dependent in self.dependents.remove(id).unwrap_or_default() {
            if let Some(prerequisites) = self.prerequisites.get_mut(&dependent) {
                prerequisites.remove(id);
            }
        }
        Some(node)
    }

    /// Adds an edge from a prerequisite to the node that depends on it.
    pub fn add_dependency(
        &mut self,
        prerequisite: &DependencyNodeId,
        dependent: &DependencyNodeId,
    ) -> Result<(), DependencyGraphError> {
        if prerequisite == dependent {
            return Err(DependencyGraphError::SelfDependency(prerequisite.clone()));
        }
        self.require_node(prerequisite)?;
        self.require_node(dependent)?;
        let inserted = self
            .prerequisites
            .get_mut(dependent)
            .expect("node indexes are maintained with nodes")
            .insert(prerequisite.clone());
        if !inserted {
            return Err(DependencyGraphError::DuplicateEdge {
                prerequisite: prerequisite.clone(),
                dependent: dependent.clone(),
            });
        }
        self.dependents
            .get_mut(prerequisite)
            .expect("node indexes are maintained with nodes")
            .insert(dependent.clone());
        Ok(())
    }

    pub fn set_resolved(
        &mut self,
        id: &DependencyNodeId,
        resolved: bool,
    ) -> Result<(), DependencyGraphError> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| DependencyGraphError::UnknownNode(id.clone()))?
            .resolved = resolved;
        Ok(())
    }

    pub fn nodes(&self) -> Vec<&DependencyNode> {
        self.nodes.values().collect()
    }

    pub fn edges(&self) -> Vec<(DependencyNodeId, DependencyNodeId)> {
        self.prerequisites
            .iter()
            .flat_map(|(dependent, prerequisites)| {
                prerequisites
                    .iter()
                    .map(move |prerequisite| (prerequisite.clone(), dependent.clone()))
            })
            .collect()
    }

    pub fn direct_prerequisites(
        &self,
        id: &DependencyNodeId,
    ) -> Result<Vec<DependencyNodeId>, DependencyGraphError> {
        self.require_node(id)?;
        Ok(self.prerequisites[id].iter().cloned().collect())
    }

    pub fn direct_dependents(
        &self,
        id: &DependencyNodeId,
    ) -> Result<Vec<DependencyNodeId>, DependencyGraphError> {
        self.require_node(id)?;
        Ok(self.dependents[id].iter().cloned().collect())
    }

    pub fn is_blocked(&self, id: &DependencyNodeId) -> Result<bool, DependencyGraphError> {
        self.require_node(id)?;
        Ok(self.prerequisites[id]
            .iter()
            .any(|prerequisite| !self.nodes[prerequisite].resolved))
    }

    pub fn validation(&self) -> DependencyGraphValidation {
        DependencyGraphValidation {
            cycles: self.cycles(),
        }
    }

    pub fn is_acyclic(&self) -> bool {
        self.cycles().is_empty()
    }

    pub fn cycles(&self) -> Vec<Vec<DependencyNodeId>> {
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut visited = BTreeSet::new();
        for id in self.nodes.keys() {
            visit(id, &self.dependents, &mut visited, &mut order);
        }

        let mut reverse_visited = BTreeSet::new();
        let mut components = Vec::new();
        for id in order.into_iter().rev() {
            if reverse_visited.contains(&id) {
                continue;
            }
            let mut component = Vec::new();
            collect_component(
                &id,
                &self.prerequisites,
                &mut reverse_visited,
                &mut component,
            );
            if component.len() > 1 {
                component.sort();
                components.push(component);
            }
        }
        components.sort();
        components
    }

    fn require_node(&self, id: &DependencyNodeId) -> Result<(), DependencyGraphError> {
        self.nodes
            .contains_key(id)
            .then_some(())
            .ok_or_else(|| DependencyGraphError::UnknownNode(id.clone()))
    }
}

fn visit(
    id: &DependencyNodeId,
    edges: &BTreeMap<DependencyNodeId, BTreeSet<DependencyNodeId>>,
    visited: &mut BTreeSet<DependencyNodeId>,
    order: &mut Vec<DependencyNodeId>,
) {
    if !visited.insert(id.clone()) {
        return;
    }
    for next in &edges[id] {
        visit(next, edges, visited, order);
    }
    order.push(id.clone());
}

fn collect_component(
    id: &DependencyNodeId,
    edges: &BTreeMap<DependencyNodeId, BTreeSet<DependencyNodeId>>,
    visited: &mut BTreeSet<DependencyNodeId>,
    component: &mut Vec<DependencyNodeId>,
) {
    if !visited.insert(id.clone()) {
        return;
    }
    component.push(id.clone());
    for next in &edges[id] {
        collect_component(next, edges, visited, component);
    }
}

#[cfg(test)]
mod tests {
    use super::{DependencyGraph, DependencyGraphError, DependencyNodeId};

    fn id(scope: &str, value: &str) -> DependencyNodeId {
        DependencyNodeId::new(scope, value)
    }

    fn graph() -> DependencyGraph {
        let mut graph = DependencyGraph::default();
        for (scope, value, resolved) in [
            ("repo-a", "build", false),
            ("repo-a", "test", false),
            ("repo-a", "lint", true),
            ("repo-b", "build", true),
        ] {
            graph
                .add_node(id(scope, value), format!("{scope}/{value}"), resolved)
                .unwrap();
        }
        graph
    }

    #[test]
    fn queries_are_sorted_and_edges_have_explicit_direction() {
        let mut graph = graph();
        graph
            .add_dependency(&id("repo-a", "lint"), &id("repo-a", "test"))
            .unwrap();
        graph
            .add_dependency(&id("repo-a", "build"), &id("repo-a", "test"))
            .unwrap();
        assert_eq!(
            graph.direct_prerequisites(&id("repo-a", "test")).unwrap(),
            vec![id("repo-a", "build"), id("repo-a", "lint")]
        );
        assert_eq!(
            graph.direct_dependents(&id("repo-a", "build")).unwrap(),
            vec![id("repo-a", "test")]
        );
        assert_eq!(
            graph.edges(),
            vec![
                (id("repo-a", "build"), id("repo-a", "test")),
                (id("repo-a", "lint"), id("repo-a", "test")),
            ]
        );
    }

    #[test]
    fn rejects_duplicate_and_self_edges() {
        let mut graph = graph();
        graph
            .add_dependency(&id("repo-a", "build"), &id("repo-a", "test"))
            .unwrap();
        assert!(matches!(
            graph.add_dependency(&id("repo-a", "build"), &id("repo-a", "test")),
            Err(DependencyGraphError::DuplicateEdge { .. })
        ));
        assert_eq!(
            graph.add_dependency(&id("repo-a", "test"), &id("repo-a", "test")),
            Err(DependencyGraphError::SelfDependency(id("repo-a", "test")))
        );
    }

    #[test]
    fn scoped_ids_keep_repositories_isolated() {
        let mut graph = graph();
        graph
            .add_dependency(&id("repo-a", "build"), &id("repo-a", "test"))
            .unwrap();
        assert!(
            graph
                .direct_dependents(&id("repo-b", "build"))
                .unwrap()
                .is_empty()
        );
        assert!(!graph.is_blocked(&id("repo-b", "build")).unwrap());
    }

    #[test]
    fn blocked_queries_follow_resolved_prerequisites() {
        let mut graph = graph();
        graph
            .add_dependency(&id("repo-a", "build"), &id("repo-a", "test"))
            .unwrap();
        assert!(graph.is_blocked(&id("repo-a", "test")).unwrap());
        graph.set_resolved(&id("repo-a", "build"), true).unwrap();
        assert!(!graph.is_blocked(&id("repo-a", "test")).unwrap());
    }

    #[test]
    fn cycles_are_reported_deterministically_without_repair() {
        let mut graph = graph();
        graph
            .add_dependency(&id("repo-a", "build"), &id("repo-a", "test"))
            .unwrap();
        graph
            .add_dependency(&id("repo-a", "test"), &id("repo-a", "lint"))
            .unwrap();
        graph
            .add_dependency(&id("repo-a", "lint"), &id("repo-a", "build"))
            .unwrap();
        assert!(!graph.is_acyclic());
        assert_eq!(
            graph.validation().cycles,
            vec![vec![
                id("repo-a", "build"),
                id("repo-a", "lint"),
                id("repo-a", "test"),
            ]]
        );
    }
}
