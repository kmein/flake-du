use std::hash::BuildHasherDefault;

use eyre::{Result, eyre};
use indexmap::IndexMap;
use parse_display::Display;
use rustc_hash::FxHasher;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use serde_with::{Map, serde_as};

/// A fully resolved flake lockfile where the root node is separated from the rest of the nodes.
pub(crate) struct Resolve {
    pub root: Node,
    pub nodes: IndexMap<String, Node, BuildHasherDefault<FxHasher>>,
}

/// Identifies a specific node in the flake lockfile.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum NodeId {
    /// The root flake itself.
    Root,
    /// A named dependency node.
    Node(String),
}

/// Raw representation of a parsed `flake.lock` file.
#[derive(Deserialize)]
pub(crate) struct Lock {
    pub root: String,
    pub nodes: IndexMap<String, Node, BuildHasherDefault<FxHasher>>,
}

/// A node in the flake lockfile graph, representing a single input and its dependencies.
#[derive(Deserialize)]
pub(crate) struct Node {
    #[serde(default)]
    pub inputs: IndexMap<String, Input, BuildHasherDefault<FxHasher>>,
    pub locked: Option<Locked>,
}

/// Represents a dependency reference from one node to another.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum Input {
    /// A direct reference to a node by its key.
    Direct(String),
    /// An alias following a path of inputs (e.g. `["nixpkgs"]`).
    Follow(Vec<String>),
}

/// The exact pinned source information for a flake input.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Locked {
    /// The type of the input (e.g., `github`, `git`, `path`).
    #[serde(rename = "type")]
    pub type_: String,
    /// The remaining fields specifying the exact locked revision/hash.
    #[serde(flatten)]
    #[serde_as(as = "Map<_, _>")]
    pub fields: Vec<(String, Value)>,
}

/// A primitive value in a locked input's field.
#[derive(Deserialize, Display)]
#[serde(untagged)]
#[display("{0}")]
pub enum Value {
    String(String),
    Bool(bool),
    Int(i64),
}

impl Locked {
    /// Transforms the dependency metadata into a JSON object compatible with `builtins.fetchTree`.
    pub(crate) fn fetch_tree_spec(&self) -> JsonMap<String, JsonValue> {
        let mut spec = JsonMap::new();
        spec.insert("type".to_string(), JsonValue::String(self.type_.clone()));

        for (key, value) in &self.fields {
            if key == "revCount" {
                continue;
            }

            spec.insert(key.clone(), value.to_json());
        }

        spec
    }
}

impl Value {
    fn to_json(&self) -> JsonValue {
        match self {
            Self::String(value) => JsonValue::String(value.clone()),
            Self::Bool(value) => JsonValue::Bool(*value),
            Self::Int(value) => JsonValue::Number(JsonNumber::from(*value)),
        }
    }
}

impl Resolve {
    /// Returns a specific node in the lockfile by its `NodeId`.
    pub(crate) fn node(&self, node_id: &NodeId) -> Option<&Node> {
        match node_id {
            NodeId::Root => Some(&self.root),
            NodeId::Node(name) => self.nodes.get(name),
        }
    }

    /// Resolves a dependency reference (`Input`) into a specific `NodeId` in the graph.
    pub(crate) fn resolve_id(&self, input: &Input) -> Option<NodeId> {
        match input {
            Input::Direct(x) => Some(NodeId::Node(x.clone())),
            Input::Follow(xs) => {
                let mut node_id = NodeId::Root;
                let mut node = &self.root;

                for x in xs {
                    let input = node.inputs.get(x)?;
                    node_id = self.resolve_id(input)?;
                    node = self.node(&node_id)?;
                }

                Some(node_id)
            }
        }
    }

    /// Convenience method to resolve an `Input` reference directly to its target `Node`.
    pub(crate) fn get(&self, input: &Input) -> Option<&Node> {
        self.node(&self.resolve_id(input)?)
    }
}

impl Lock {
    /// Resolves the raw lockfile structure into a traversable graph.
    pub(crate) fn resolve(mut self) -> Result<Resolve> {
        Ok(Resolve {
            root: self
                .nodes
                .swap_remove(&self.root)
                .ok_or_else(|| eyre!("no root node"))?,
            nodes: self.nodes,
        })
    }
}
