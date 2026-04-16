use std::collections::HashSet;

use eyre::{Context, Result};
use ptree::{TreeBuilder, write_tree};

use crate::{
    lock::{Input, Resolve},
    size::{SizeIndex, format_bytes},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TreeRenderOptions {
    pub show_cumulative_size: bool,
    pub show_store_paths: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SizeEstimate {
    known: u64,
    has_unknown: bool,
}

impl SizeEstimate {
    fn to_option(self) -> Option<u64> {
        if self.has_unknown {
            None
        } else {
            Some(self.known)
        }
    }
}

pub(crate) fn render_tree_text(
    lock: &Resolve,
    sizes: &SizeIndex,
    options: TreeRenderOptions,
) -> Result<String> {
    let mut builder = TreeBuilder::new(root_label(lock, sizes, options));
    append_children(lock, sizes, &mut builder, &Input::Follow(Vec::new()), options);

    let tree = builder.build();
    let mut out = Vec::new();
    write_tree(&tree, &mut out).context("failed to render static tree view")?;

    String::from_utf8(out).context("tree output was not valid UTF-8")
}

fn append_children(
    lock: &Resolve,
    sizes: &SizeIndex,
    builder: &mut TreeBuilder,
    cursor: &Input,
    options: TreeRenderOptions,
) {
    let Some(node) = lock.get(cursor) else {
        return;
    };

    let mut children: Vec<_> = node.inputs.iter().collect();
    children.sort_by_key(|(_, input)| {
        let estimate = subtree_size(lock, sizes, input, true);
        std::cmp::Reverse(estimate.known)
    });

    for (name, input) in children {
        let label = tree_label(lock, sizes, name, input, options);
        let has_children = matches!(input, Input::Direct(_))
            && lock.get(input).is_some_and(|child| !child.inputs.is_empty());

        if has_children {
            builder.begin_child(label);
            append_children(lock, sizes, builder, input, options);
            builder.end_child();
        } else {
            builder.add_empty_child(label);
        }
    }
}

fn root_label(lock: &Resolve, sizes: &SizeIndex, options: TreeRenderOptions) -> String {
    if options.show_cumulative_size {
        let estimate = subtree_size(lock, sizes, &Input::Follow(Vec::new()), false);
        let total = format_size_estimate(estimate);
        format!("inputs [Σ {total}]")
    } else {
        "inputs".to_string()
    }
}

fn tree_label(
    lock: &Resolve,
    sizes: &SizeIndex,
    name: &str,
    input: &Input,
    options: TreeRenderOptions,
) -> String {
    let own_size = match input {
        Input::Direct(_) => lock.resolve_id(input).as_ref().and_then(|node_id| sizes.size(node_id)),
        Input::Follow(_) => Some(0),
    };
    let total_estimate = subtree_size(lock, sizes, input, true);
    let suffix = format_size_suffix(own_size, total_estimate, options)
        .map(|suffix| format!(" {suffix}"))
        .unwrap_or_default();

    let path_suffix = if options.show_store_paths {
        match input {
            Input::Direct(_) => lock.resolve_id(input).as_ref()
                .and_then(|node_id| sizes.path(node_id))
                .map(|path| format!(" \x1b[90m({path})\x1b[0m"))
                .unwrap_or_default(),
            Input::Follow(_) => String::new(),
        }
    } else {
        String::new()
    };

    match input {
        Input::Direct(x) => {
            if x == name {
                format!("{name}{suffix}{path_suffix}")
            } else {
                format!("{name}: {x}{suffix}{path_suffix}")
            }
        }
        Input::Follow(xs) => {
            let target = if xs.is_empty() {
                "<self>".to_string()
            } else {
                xs.join("/")
            };

            format!("{name} -> {target}{suffix}{path_suffix}")
        }
    }
}

fn format_size_suffix(
    own_size: Option<u64>,
    total_estimate: SizeEstimate,
    options: TreeRenderOptions,
) -> Option<String> {
    let mut parts = Vec::new();

    parts.push(format_bytes(own_size));

    if options.show_cumulative_size && own_size != total_estimate.to_option() {
        parts.push(format!("Σ {}", format_size_estimate(total_estimate)));
    }

    (!parts.is_empty()).then(|| format!("[{}]", parts.join(", ")))
}

fn format_size_estimate(estimate: SizeEstimate) -> String {
    if estimate.has_unknown {
        format!("≥ {}", format_bytes(Some(estimate.known)))
    } else {
        format_bytes(Some(estimate.known))
    }
}

pub(crate) fn subtree_size(
    lock: &Resolve,
    sizes: &SizeIndex,
    cursor: &Input,
    include_self: bool,
) -> SizeEstimate {
    let mut seen = HashSet::new();
    let mut total = 0;
    let mut missing = false;

    collect_subtree_sizes(
        lock,
        sizes,
        cursor,
        include_self,
        &mut seen,
        &mut total,
        &mut missing,
    );

    SizeEstimate {
        known: total,
        has_unknown: missing,
    }
}

fn collect_subtree_sizes(
    lock: &Resolve,
    sizes: &SizeIndex,
    cursor: &Input,
    include_self: bool,
    seen: &mut HashSet<String>,
    total: &mut u64,
    missing: &mut bool,
) {
    if include_self {
        if matches!(cursor, Input::Follow(_)) {
            return;
        }

        match lock.resolve_id(cursor) {
            Some(node_id) => match sizes.path(&node_id) {
                Some(path) => {
                    if seen.insert(path.to_string()) {
                        if let Some(size) = sizes.size(&node_id) {
                            *total += size;
                        } else {
                            *missing = true;
                        }
                    }
                }
                None => *missing = true,
            },
            None => *missing = true,
        }
    }

    let Some(node) = lock.get(cursor) else {
        return;
    };

    for input in node.inputs.values() {
        collect_subtree_sizes(lock, sizes, input, true, seen, total, missing);
    }
}

#[cfg(test)]
mod tests {
    use std::hash::BuildHasherDefault;

    use indexmap::IndexMap;
    use rustc_hash::FxHasher;

    use super::{TreeRenderOptions, render_tree_text};
    use crate::{
        lock::{Input, Node, Resolve},
        size::SizeIndex,
    };

    #[test]
    fn renders_self_and_cumulative_sizes() {
        let lock = Resolve {
            root: Node {
                inputs: IndexMap::from_iter([
                    ("alias".to_string(), Input::Follow(vec!["base".to_string()])),
                    ("base".to_string(), Input::Direct("base".to_string())),
                ]),
                locked: None,
            },
            nodes: IndexMap::from_iter([
                (
                    "base".to_string(),
                    Node {
                        inputs: IndexMap::from_iter([(
                            "child".to_string(),
                            Input::Direct("child".to_string()),
                        )]),
                        locked: None,
                    },
                ),
                (
                    "child".to_string(),
                    Node {
                        inputs: IndexMap::<String, Input, BuildHasherDefault<FxHasher>>::default(),
                        locked: None,
                    },
                ),
            ]),
        };
        let sizes = SizeIndex::from_test_sizes([
            ("base", "/nix/store/base", 42),
            ("child", "/nix/store/child", 84),
        ]);

        let rendered = render_tree_text(
            &lock,
            &sizes,
            TreeRenderOptions {
                show_cumulative_size: true,
                show_store_paths: false,
            },
        )
        .unwrap();

        assert!(rendered.contains("inputs [Σ 126 B]"));
        assert!(rendered.contains("├─ base [42 B, Σ 126 B]"));
        assert!(rendered.contains("│  └─ child [84 B]"));
        assert!(rendered.contains("└─ alias -> base [0 B]"));
    }

    #[test]
    fn can_hide_cumulative_sizes() {
        let lock = Resolve {
            root: Node {
                inputs: IndexMap::from_iter([("base".to_string(), Input::Direct("base".to_string()))]),
                locked: None,
            },
            nodes: IndexMap::from_iter([(
                "base".to_string(),
                Node {
                    inputs: IndexMap::<String, Input, BuildHasherDefault<FxHasher>>::default(),
                    locked: None,
                },
            )]),
        };
        let sizes = SizeIndex::from_test_sizes([("base", "/nix/store/base", 42)]);

        let rendered = render_tree_text(
            &lock,
            &sizes,
            TreeRenderOptions {
                show_cumulative_size: false,
                show_store_paths: false,
            },
        )
        .unwrap();

        assert!(rendered.contains("└─ base [42 B]"));
        assert!(!rendered.contains(", "));
    }
}
