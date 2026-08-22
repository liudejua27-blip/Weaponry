//! ForgeCAD-owned feature dependency graph primitives.
//!
//! This module is deliberately independent of the geometry compiler, Runtime
//! persistence, and any external CAD system.  A feature is identified by its
//! caller-owned opaque `feature_id`; the graph never derives identity from a
//! vector position or from a generated mesh.  That makes a feature's identity
//! stable when the authoring declaration is reordered and lets Runtime keep a
//! bounded local edit scoped to the affected feature closure.
//!
//! The graph is a validation/planning primitive only.  It does not compile
//! geometry, execute scripts, write SQLite/CAS, or claim that a recompute has
//! succeeded.  Callers must still run the normal typed Worker and readback
//! gates for every feature in the returned plan.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

const MAX_FEATURE_ID_LENGTH: usize = 128;
const MAX_FEATURE_NODES: usize = 512;
const MAX_FEATURE_EDGES: usize = 2048;

/// A stable, ForgeCAD-owned feature identity.
///
/// IDs intentionally use the same closed ASCII grammar as the typed geometry
/// identifiers.  In particular, paths, expressions, and generated indexes
/// are not accepted as an identity source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(String);

impl FeatureId {
    /// Validate and retain a caller-owned feature ID.
    pub fn new(value: impl AsRef<str>) -> Result<Self, FeatureIdError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(FeatureIdError::Empty);
        }
        if value.len() > MAX_FEATURE_ID_LENGTH {
            return Err(FeatureIdError::TooLong);
        }
        if let Some(character) = value.chars().find(|character| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '_' | '-')
        }) {
            return Err(FeatureIdError::InvalidCharacter(character));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for FeatureId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for FeatureId {
    type Error = FeatureIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for FeatureId {
    type Error = FeatureIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FeatureId> for String {
    fn from(value: FeatureId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureIdError {
    Empty,
    TooLong,
    InvalidCharacter(char),
}

impl fmt::Display for FeatureIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("feature_id must not be empty"),
            Self::TooLong => write!(
                formatter,
                "feature_id must be at most {MAX_FEATURE_ID_LENGTH} bytes"
            ),
            Self::InvalidCharacter(character) => {
                write!(
                    formatter,
                    "feature_id contains invalid character {character:?}"
                )
            }
        }
    }
}

impl std::error::Error for FeatureIdError {}

/// One feature and its direct dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureNode {
    pub feature_id: FeatureId,
    pub dependencies: Vec<FeatureId>,
}

impl FeatureNode {
    /// Build a node from strings or already validated `FeatureId`s.
    ///
    /// Dependency order is canonicalized by ID.  Repeating a dependency is a
    /// malformed declaration and is rejected rather than silently deduplicated.
    pub fn new<F, I, D>(feature_id: F, dependencies: I) -> Result<Self, FeatureGraphError>
    where
        F: AsRef<str>,
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
    {
        let feature_id = FeatureId::new(feature_id).map_err(FeatureGraphError::InvalidId)?;
        let mut parsed = Vec::new();
        let mut seen = BTreeSet::new();
        for dependency in dependencies {
            let dependency = FeatureId::new(dependency).map_err(FeatureGraphError::InvalidId)?;
            if !seen.insert(dependency.clone()) {
                return Err(FeatureGraphError::DuplicateDependency {
                    feature_id,
                    dependency,
                });
            }
            parsed.push(dependency);
        }
        parsed.sort();
        Ok(Self {
            feature_id,
            dependencies: parsed,
        })
    }

    pub fn leaf<F>(feature_id: F) -> Result<Self, FeatureGraphError>
    where
        F: AsRef<str>,
    {
        Self::new(feature_id, std::iter::empty::<&str>())
    }
}

/// Validation and planning failures.  All malformed graph and plan inputs
/// are returned as errors; no partial plan is exposed to a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureGraphError {
    InvalidId(FeatureIdError),
    NodeBudgetExceeded {
        count: usize,
        maximum: usize,
    },
    EdgeBudgetExceeded {
        count: usize,
        maximum: usize,
    },
    DuplicateFeatureId(FeatureId),
    DuplicateDependency {
        feature_id: FeatureId,
        dependency: FeatureId,
    },
    UnknownDependency {
        feature_id: FeatureId,
        dependency: FeatureId,
    },
    Cycle {
        feature_ids: Vec<FeatureId>,
    },
    UnknownDirtyFeature(FeatureId),
}

impl fmt::Display for FeatureGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(error) => write!(formatter, "invalid feature_id: {error}"),
            Self::NodeBudgetExceeded { count, maximum } => {
                write!(
                    formatter,
                    "feature node budget exceeded: {count} > {maximum}"
                )
            }
            Self::EdgeBudgetExceeded { count, maximum } => {
                write!(
                    formatter,
                    "feature edge budget exceeded: {count} > {maximum}"
                )
            }
            Self::DuplicateFeatureId(feature_id) => {
                write!(formatter, "duplicate feature_id: {feature_id}")
            }
            Self::DuplicateDependency {
                feature_id,
                dependency,
            } => write!(
                formatter,
                "feature {feature_id} repeats dependency {dependency}"
            ),
            Self::UnknownDependency {
                feature_id,
                dependency,
            } => write!(
                formatter,
                "feature {feature_id} depends on unknown feature {dependency}"
            ),
            Self::Cycle { feature_ids } => {
                write!(formatter, "feature dependency cycle: {feature_ids:?}")
            }
            Self::UnknownDirtyFeature(feature_id) => {
                write!(
                    formatter,
                    "dirty seed references unknown feature {feature_id}"
                )
            }
        }
    }
}

impl std::error::Error for FeatureGraphError {}

/// A validated acyclic graph with deterministic traversal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureGraph {
    nodes: BTreeMap<FeatureId, FeatureNode>,
    dependents: BTreeMap<FeatureId, BTreeSet<FeatureId>>,
    topological_order: Vec<FeatureId>,
}

impl FeatureGraph {
    /// Validate feature identity, dependency references, and acyclicity.
    pub fn new<I>(nodes: I) -> Result<Self, FeatureGraphError>
    where
        I: IntoIterator<Item = FeatureNode>,
    {
        let mut by_id = BTreeMap::new();
        for node in nodes {
            let mut node = node;
            node.dependencies.sort();
            let feature_id = node.feature_id.clone();
            if by_id.contains_key(&feature_id) {
                return Err(FeatureGraphError::DuplicateFeatureId(feature_id));
            }
            for dependencies in node.dependencies.windows(2) {
                if dependencies[0] == dependencies[1] {
                    return Err(FeatureGraphError::DuplicateDependency {
                        feature_id: feature_id.clone(),
                        dependency: dependencies[0].clone(),
                    });
                }
            }
            by_id.insert(feature_id, node);
            if by_id.len() > MAX_FEATURE_NODES {
                return Err(FeatureGraphError::NodeBudgetExceeded {
                    count: by_id.len(),
                    maximum: MAX_FEATURE_NODES,
                });
            }
        }

        let edge_count = by_id
            .values()
            .map(|node| node.dependencies.len())
            .sum::<usize>();
        if edge_count > MAX_FEATURE_EDGES {
            return Err(FeatureGraphError::EdgeBudgetExceeded {
                count: edge_count,
                maximum: MAX_FEATURE_EDGES,
            });
        }

        let mut dependents = by_id
            .keys()
            .cloned()
            .map(|feature_id| (feature_id, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for (feature_id, node) in &by_id {
            for dependency in &node.dependencies {
                if !by_id.contains_key(dependency) {
                    return Err(FeatureGraphError::UnknownDependency {
                        feature_id: feature_id.clone(),
                        dependency: dependency.clone(),
                    });
                }
                dependents
                    .get_mut(dependency)
                    .expect("every validated dependency has a reverse entry")
                    .insert(feature_id.clone());
            }
        }

        let topological_order = topological_order(&by_id, &dependents)?;
        Ok(Self {
            nodes: by_id,
            dependents,
            topological_order,
        })
    }

    pub fn nodes(&self) -> &BTreeMap<FeatureId, FeatureNode> {
        &self.nodes
    }

    pub fn node(&self, feature_id: impl AsRef<str>) -> Option<&FeatureNode> {
        let feature_id = FeatureId::new(feature_id).ok()?;
        self.nodes.get(&feature_id)
    }

    pub fn topological_order(&self) -> &[FeatureId] {
        &self.topological_order
    }

    /// Return the dirty seed closure: each requested feature plus all of its
    /// transitive dependents.  BTree collections make the returned IDs
    /// independent of declaration order and hash-map iteration order.
    pub fn dirty_propagation<I, D>(
        &self,
        dirty_features: I,
    ) -> Result<Vec<FeatureId>, FeatureGraphError>
    where
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
    {
        let mut queue = VecDeque::new();
        let mut affected = BTreeSet::new();
        for dirty_feature in dirty_features {
            let dirty_feature =
                FeatureId::new(dirty_feature).map_err(FeatureGraphError::InvalidId)?;
            if !self.nodes.contains_key(&dirty_feature) {
                return Err(FeatureGraphError::UnknownDirtyFeature(dirty_feature));
            }
            if affected.insert(dirty_feature.clone()) {
                queue.push_back(dirty_feature);
            }
        }

        while let Some(feature_id) = queue.pop_front() {
            if let Some(dependents) = self.dependents.get(&feature_id) {
                for dependent in dependents {
                    if affected.insert(dependent.clone()) {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
        Ok(affected.into_iter().collect())
    }

    /// Produce a deterministic, dependency-first recompute plan for one
    /// bounded edit.  Unaffected features are intentionally omitted; a caller
    /// must not infer that they were recomputed from this result.
    pub fn recompute_plan<I, D>(
        &self,
        dirty_features: I,
    ) -> Result<RecomputePlan, FeatureGraphError>
    where
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
    {
        let dirty_features = self.dirty_propagation(dirty_features)?;
        let dirty_set = dirty_features.iter().cloned().collect::<BTreeSet<_>>();
        let recompute_order = self
            .topological_order
            .iter()
            .filter(|feature_id| dirty_set.contains(*feature_id))
            .cloned()
            .collect();
        Ok(RecomputePlan {
            dirty_features,
            recompute_order,
        })
    }
}

/// The complete dirty closure and dependency-first order for one local edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecomputePlan {
    pub dirty_features: Vec<FeatureId>,
    pub recompute_order: Vec<FeatureId>,
}

impl RecomputePlan {
    pub fn is_empty(&self) -> bool {
        self.dirty_features.is_empty()
    }

    pub fn dirty_features(&self) -> &[FeatureId] {
        &self.dirty_features
    }

    pub fn recompute_order(&self) -> &[FeatureId] {
        &self.recompute_order
    }
}

fn topological_order(
    nodes: &BTreeMap<FeatureId, FeatureNode>,
    dependents: &BTreeMap<FeatureId, BTreeSet<FeatureId>>,
) -> Result<Vec<FeatureId>, FeatureGraphError> {
    let mut indegree = nodes
        .iter()
        .map(|(feature_id, node)| (feature_id.clone(), node.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(feature_id, _)| feature_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(nodes.len());

    while let Some(feature_id) = take_first(&mut ready) {
        order.push(feature_id.clone());
        if let Some(children) = dependents.get(&feature_id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("every dependent is a graph node");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }

    if order.len() != nodes.len() {
        let feature_ids = find_cycle(nodes).unwrap_or_else(|| {
            nodes
                .keys()
                .filter(|feature_id| !order.contains(feature_id))
                .cloned()
                .collect()
        });
        return Err(FeatureGraphError::Cycle { feature_ids });
    }
    Ok(order)
}

fn take_first<T>(values: &mut BTreeSet<T>) -> Option<T>
where
    T: Ord + Clone,
{
    let first = values.iter().next()?.clone();
    values.remove(&first);
    Some(first)
}

fn find_cycle(nodes: &BTreeMap<FeatureId, FeatureNode>) -> Option<Vec<FeatureId>> {
    fn visit(
        feature_id: &FeatureId,
        nodes: &BTreeMap<FeatureId, FeatureNode>,
        colors: &mut BTreeMap<FeatureId, u8>,
        stack: &mut Vec<FeatureId>,
        stack_positions: &mut BTreeMap<FeatureId, usize>,
    ) -> Option<Vec<FeatureId>> {
        colors.insert(feature_id.clone(), 1);
        stack_positions.insert(feature_id.clone(), stack.len());
        stack.push(feature_id.clone());
        let node = nodes
            .get(feature_id)
            .expect("cycle detection only visits graph nodes");
        for dependency in &node.dependencies {
            match colors.get(dependency).copied().unwrap_or(0) {
                0 => {
                    if let Some(cycle) = visit(dependency, nodes, colors, stack, stack_positions) {
                        return Some(cycle);
                    }
                }
                1 => {
                    let start = *stack_positions
                        .get(dependency)
                        .expect("a visiting node has a stack position");
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(dependency.clone());
                    return Some(cycle);
                }
                _ => {}
            }
        }
        stack.pop();
        stack_positions.remove(feature_id);
        colors.insert(feature_id.clone(), 2);
        None
    }

    let mut colors = BTreeMap::new();
    let mut stack = Vec::new();
    let mut stack_positions = BTreeMap::new();
    for feature_id in nodes.keys() {
        if colors.get(feature_id).copied().unwrap_or(0) == 0 {
            if let Some(cycle) = visit(
                feature_id,
                nodes,
                &mut colors,
                &mut stack,
                &mut stack_positions,
            ) {
                return Some(cycle);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(feature_id: &str, dependencies: &[&str]) -> FeatureNode {
        FeatureNode::new(feature_id, dependencies.iter().copied()).expect("valid test node")
    }

    #[test]
    fn feature_ids_are_stable_across_declaration_order() {
        let first = FeatureGraph::new([
            node("panel", &["base"]),
            node("base", &[]),
            node("trim", &["panel"]),
        ])
        .expect("acyclic graph");
        let second = FeatureGraph::new([
            node("trim", &["panel"]),
            node("base", &[]),
            node("panel", &["base"]),
        ])
        .expect("acyclic graph");

        assert_eq!(first.topological_order(), second.topological_order());
        assert_eq!(first.node("panel").unwrap().feature_id.as_str(), "panel");
    }

    #[test]
    fn invalid_ids_and_duplicate_nodes_fail_closed() {
        assert!(matches!(
            FeatureId::new("feature/with/path"),
            Err(FeatureIdError::InvalidCharacter('/'))
        ));
        let duplicate = FeatureGraph::new([node("base", &[]), node("base", &[])]);
        assert!(matches!(
            duplicate,
            Err(FeatureGraphError::DuplicateFeatureId(feature_id))
                if feature_id.as_str() == "base"
        ));

        let base = FeatureId::new("base").expect("valid id");
        let duplicate_dependency = FeatureGraph::new([FeatureNode {
            feature_id: FeatureId::new("panel").expect("valid id"),
            dependencies: vec![base.clone(), base],
        }]);
        assert!(matches!(
            duplicate_dependency,
            Err(FeatureGraphError::DuplicateDependency { .. })
        ));
    }

    #[test]
    fn unknown_dependencies_fail_closed() {
        let graph = FeatureGraph::new([node("panel", &["missing"])]);
        assert!(matches!(
            graph,
            Err(FeatureGraphError::UnknownDependency { feature_id, dependency })
                if feature_id.as_str() == "panel" && dependency.as_str() == "missing"
        ));
    }

    #[test]
    fn self_and_multi_node_cycles_fail_closed() {
        let self_cycle = FeatureGraph::new([node("loop", &["loop"])]);
        assert!(matches!(self_cycle, Err(FeatureGraphError::Cycle { .. })));

        let multi_cycle =
            FeatureGraph::new([node("a", &["b"]), node("b", &["a"]), node("leaf", &["a"])]);
        assert!(matches!(multi_cycle, Err(FeatureGraphError::Cycle { .. })));
    }

    #[test]
    fn topological_order_is_dependency_first_and_tie_broken_by_id() {
        let graph = FeatureGraph::new([
            node("final", &["rib", "panel"]),
            node("panel", &["base"]),
            node("rib", &["base"]),
            node("base", &[]),
        ])
        .expect("acyclic graph");
        let ids = graph
            .topological_order()
            .iter()
            .map(FeatureId::as_str)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["base", "panel", "rib", "final"]);
    }

    #[test]
    fn dirty_propagation_is_transitive_and_excludes_unaffected_branches() {
        let graph = FeatureGraph::new([
            node("base", &[]),
            node("panel", &["base"]),
            node("rib", &["base"]),
            node("final", &["panel", "rib"]),
            node("unrelated", &[]),
        ])
        .expect("acyclic graph");

        let dirty = graph.dirty_propagation(["rib"]).expect("known seed");
        assert_eq!(
            dirty.iter().map(FeatureId::as_str).collect::<Vec<_>>(),
            ["final", "rib"]
        );
    }

    #[test]
    fn recompute_plan_is_deterministic_and_dependency_first() {
        let graph = FeatureGraph::new([
            node("base", &[]),
            node("panel", &["base"]),
            node("rib", &["base"]),
            node("final", &["panel", "rib"]),
            node("unrelated", &[]),
        ])
        .expect("acyclic graph");

        let plan = graph
            .recompute_plan(["rib", "panel", "rib"])
            .expect("known seeds");
        assert_eq!(
            plan.dirty_features()
                .iter()
                .map(FeatureId::as_str)
                .collect::<Vec<_>>(),
            ["final", "panel", "rib"]
        );
        assert_eq!(
            plan.recompute_order()
                .iter()
                .map(FeatureId::as_str)
                .collect::<Vec<_>>(),
            ["panel", "rib", "final"]
        );
    }

    #[test]
    fn unknown_dirty_seed_fails_closed_and_empty_seed_is_noop() {
        let graph = FeatureGraph::new([node("base", &[])]).expect("acyclic graph");
        assert!(matches!(
            graph.dirty_propagation(["missing"]),
            Err(FeatureGraphError::UnknownDirtyFeature(feature_id))
                if feature_id.as_str() == "missing"
        ));
        let plan = graph
            .recompute_plan(std::iter::empty::<&str>())
            .expect("empty plan");
        assert!(plan.is_empty());
        assert!(plan.recompute_order().is_empty());
    }

    #[test]
    fn graph_budgets_fail_closed_before_recursive_cycle_diagnostics() {
        let too_many_nodes = (0..=MAX_FEATURE_NODES)
            .map(|index| FeatureNode::leaf(format!("feature-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .expect("valid bounded IDs");
        assert!(matches!(
            FeatureGraph::new(too_many_nodes),
            Err(FeatureGraphError::NodeBudgetExceeded { .. })
        ));

        let dense_nodes = (0..66)
            .map(|index| {
                let dependencies = (0..index)
                    .map(|dependency| format!("dense-{dependency}"))
                    .collect::<Vec<_>>();
                FeatureNode::new(format!("dense-{index}"), dependencies)
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("valid dense graph declarations");
        assert!(matches!(
            FeatureGraph::new(dense_nodes),
            Err(FeatureGraphError::EdgeBudgetExceeded { .. })
        ));
    }
}
