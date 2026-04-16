use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    process::Command,
};

use eyre::{Context, Result, eyre};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::lock::{Input, Locked, NodeId, Resolve};

/// A representation of the computed Nix store size metrics for a node.
#[derive(Clone, Debug, Default)]
pub(crate) struct NodeSize {
    /// The computed hash or path of the input source tree in the Nix store.
    pub path: Option<String>,
    /// The size of the closure downloaded to evaluate this node, in bytes.
    pub size: Option<u64>,
}

/// An index built by examining the exact derivations/nix store paths the locked inputs fetch.
#[derive(Default)]
pub(crate) struct SizeIndex {
    by_node: HashMap<NodeId, NodeSize>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ArchivedFlake {
    path: String,
    #[serde(default)]
    inputs: IndexMap<String, ArchivedFlake>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathInfo {
    path: Option<String>,
    closure_size: Option<u64>,
    nar_size: Option<u64>,
    valid: Option<bool>,
}

impl SizeIndex {
    pub(crate) fn load(lock: &Resolve, flake_path: &Path, lock_path: &Path) -> Self {
        match Self::try_load(lock, flake_path, lock_path) {
            Ok(index) => index,
            Err(err) => Self {
                by_node: HashMap::new(),
                error: Some(err.to_string()),
            },
        }
    }

    fn try_load(lock: &Resolve, flake_path: &Path, lock_path: &Path) -> Result<Self> {
        let archived = archive_flake(flake_path, lock_path)?;
        let mut by_node = HashMap::new();

        collect_paths(lock, &Input::Follow(Vec::new()), &archived, &mut by_node);

        let loaded = load_sizes(lock, &mut by_node)?;

        Ok(Self {
            by_node: by_node
                .into_iter()
                .map(|(node_id, path)| {
                    let size = loaded
                        .sizes
                        .get(&path)
                        .and_then(|info| info.closure_size.or(info.nar_size));

                    (
                        node_id,
                        NodeSize {
                            path: Some(path),
                            size,
                        },
                    )
                })
                .collect(),
            error: loaded.warning,
        })
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn path(&self, node_id: &NodeId) -> Option<&str> {
        self.by_node.get(node_id).and_then(|node| node.path.as_deref())
    }

    pub(crate) fn size(&self, node_id: &NodeId) -> Option<u64> {
        self.by_node.get(node_id).and_then(|node| node.size)
    }

    #[cfg(test)]
    pub(crate) fn from_test_sizes<const N: usize>(entries: [(&str, &str, u64); N]) -> Self {
        Self {
            by_node: entries
                .into_iter()
                .map(|(name, path, size)| {
                    (
                        NodeId::Node(name.to_string()),
                        NodeSize {
                            path: Some(path.to_string()),
                            size: Some(size),
                        },
                    )
                })
                .collect(),
            error: None,
        }
    }
}

fn archive_flake(flake_path: &Path, lock_path: &Path) -> Result<ArchivedFlake> {
    debug!("running nix flake archive on {}", flake_path.display());
    let output = Command::new("nix")
        .arg("--quiet")
        .args([
            "flake",
            "archive",
            "--json",
            "--dry-run",
            "--no-write-lock-file",
            "--no-update-lock-file",
            "--reference-lock-file",
        ])
        .arg(lock_path)
        .arg(flake_path)
        .output()
        .with_context(|| format!("failed to run nix flake archive for {}", flake_path.display()))?;

    if !output.status.success() {
        let err = summarize_nix_stderr(&output.stderr);
        warn!("nix flake archive failed: {}", err);
        return Err(eyre!("nix flake archive failed: {}", err));
    }

    decode_nix_json(&output.stdout, "failed to decode nix flake archive output")
}

struct LoadedSizes {
    sizes: HashMap<String, PathInfo>,
    warning: Option<String>,
}

struct QueryPathInfo {
    sizes: HashMap<String, PathInfo>,
    warning: Option<String>,
}

fn load_sizes(lock: &Resolve, by_node: &mut HashMap<NodeId, String>) -> Result<LoadedSizes> {
    if by_node.is_empty() {
        return Ok(LoadedSizes {
            sizes: HashMap::new(),
            warning: None,
        });
    }

    let paths = by_node
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut fallback_error = None;
    let mut sizes = match query_path_info(&paths) {
        Ok(info) => info.sizes,
        Err(batch_error) => {
            fallback_error = Some(batch_error.to_string());

            let mut sizes = HashMap::new();
            for path in &paths {
                if let Ok(info) = query_path_info(std::slice::from_ref(path)) {
                    sizes.extend(info.sizes);
                }
            }

            if sizes.is_empty() {
                return Err(batch_error);
            }

            sizes
        }
    };

    let mut failures = Vec::new();
    let missing_groups = missing_node_groups(by_node, &sizes);
    
    if !missing_groups.is_empty() {
        debug!("fetching {} missing node groups individually", missing_groups.len());
    }

    for node_ids in missing_groups.into_values() {
        let Some(node_id) = node_ids.first().cloned() else {
            continue;
        };

        let realized_path = match realize_node_path(lock, &node_id) {
            Ok(path) => path,
            Err(err) => {
                warn!("failed to realize node path for {:?}: {}", node_id, err);
                failures.push(err.to_string());
                continue;
            }
        };

        for node_id in &node_ids {
            by_node.insert(node_id.clone(), realized_path.clone());
        }

        match query_path_info(std::slice::from_ref(&realized_path)) {
            Ok(info) if !info.sizes.is_empty() => {
                sizes.extend(info.sizes);
            }
            Ok(info) => {
                let msg = info.warning
                    .unwrap_or_else(|| "failed to query realized input size".to_string());
                warn!("query_path_info for realized path returned success but with warning: {}", msg);
                failures.push(msg);
            }
            Err(err) => {
                warn!("query_path_info failed for realized path: {}", err);
                failures.push(err.to_string());
            }
        }
    }

    let unavailable = by_node
        .values()
        .filter(|path| !sizes.contains_key(*path))
        .count();
    let warning = if unavailable == 0 {
        None
    } else {
        let summary = failures
            .first()
            .cloned()
            .or(fallback_error)
            .unwrap_or_else(|| "failed to fetch some locked inputs".to_string());
        Some(format!("{unavailable} input sizes unavailable; {summary}"))
    };

    Ok(LoadedSizes { sizes, warning })
}

fn query_path_info(paths: &[String]) -> Result<QueryPathInfo> {
    debug!("running nix path-info on {} paths", paths.len());
    let output = Command::new("nix")
        .arg("--quiet")
        .args(["path-info", "--json", "--closure-size"])
        .args(paths)
        .output()
        .context("failed to run nix path-info")?;

    if !output.status.success() {
        let err = summarize_nix_stderr(&output.stderr);
        warn!("nix path-info failed: {}", err);
        return Err(eyre!("nix path-info failed: {}", err));
    }

    let (sizes, missing) = if let Ok(array) = decode_nix_json::<Vec<PathInfo>>(&output.stdout, "failed to decode nix path-info output (array format)") {
        let missing = array.iter().filter(|info| info.path.is_none() || info.valid == Some(false)).count();
        let sizes = array
            .into_iter()
            .filter(|info| info.valid != Some(false))
            .filter_map(|mut info| info.path.take().map(|path| (path, info)))
            .collect::<HashMap<_, _>>();
        (sizes, missing)
    } else {
        let decoded = decode_nix_json::<HashMap<String, Option<PathInfo>>>(
            &output.stdout,
            "failed to decode nix path-info output (object format)",
        )?;
        let missing = decoded.values().filter(|info| info.as_ref().map_or(true, |info| info.valid == Some(false))).count();
        let sizes = decoded
            .into_iter()
            .filter_map(|(path, info)| info.filter(|info| info.valid != Some(false)).map(|info| (path, info)))
            .collect::<HashMap<_, _>>();
        (sizes, missing)
    };

    Ok(QueryPathInfo {
        sizes,
        warning: (missing > 0).then(|| summarize_missing_path_info(&output.stderr)),
    })
}

fn collect_paths(
    lock: &Resolve,
    cursor: &Input,
    archived: &ArchivedFlake,
    by_node: &mut HashMap<NodeId, String>,
) {
    let Some(node) = lock.get(cursor) else {
        return;
    };

    for (name, input) in &node.inputs {
        if matches!(input, Input::Follow(_)) {
            continue;
        }

        let Some(child) = archived.inputs.get(name) else {
            continue;
        };

        if let Some(node_id) = lock.resolve_id(input) {
            by_node.entry(node_id).or_insert_with(|| child.path.clone());
        }

        collect_paths(lock, input, child, by_node);
    }
}

fn missing_node_groups(
    by_node: &HashMap<NodeId, String>,
    sizes: &HashMap<String, PathInfo>,
) -> BTreeMap<String, Vec<NodeId>> {
    let mut missing = BTreeMap::new();

    for (node_id, path) in by_node {
        if !sizes.contains_key(path) {
            missing
                .entry(path.clone())
                .or_insert_with(Vec::new)
                .push(node_id.clone());
        }
    }

    missing
}

fn realize_node_path(lock: &Resolve, node_id: &NodeId) -> Result<String> {
    let locked = lock
        .node(node_id)
        .and_then(|node| node.locked.as_ref())
        .ok_or_else(|| eyre!("missing locked metadata for unresolved input"))?;

    realize_locked_path(locked)
}

fn realize_locked_path(locked: &Locked) -> Result<String> {
    let spec = serde_json::to_string(&locked.fetch_tree_spec())
        .context("failed to encode fetchTree input specification")?;
    let expr = format!(
        "let spec = builtins.fromJSON {}; in (builtins.fetchTree spec).outPath",
        serde_json::to_string(&spec).context("failed to escape fetchTree input specification")?
    );

    debug!("realizing missing input {} with builtins.fetchTree", spec);
    let output = Command::new("nix")
        .arg("--quiet")
        .args(["eval", "--raw", "--expr"])
        .arg(expr)
        .output()
        .context("failed to run nix eval for builtins.fetchTree")?;

    if !output.status.success() {
        let err = summarize_nix_stderr(&output.stderr);
        warn!("nix fetchTree failed: {}", err);
        return Err(eyre!(
            "nix fetchTree failed: {}", err
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Err(eyre!("nix fetchTree returned an empty store path"))
    } else {
        Ok(path)
    }
}

/// Formats the size into human-readable bytes (e.g. `14.2 MiB`).
pub(crate) fn format_bytes(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "?".to_string();
    };

    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{size} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

fn summarize_nix_stderr(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let lines = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !line.starts_with("warning:")
                && !line.starts_with("unpacking ")
                && !line.starts_with("copying path ")
        })
        .collect::<Vec<_>>();

    let fetch = lines
        .iter()
        .find_map(|line| extract_between(line, "while fetching the input '", "'"));
    let detail = lines
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix("error:").map(str::trim))
        .filter(|line| !line.is_empty())
        .or_else(|| lines.last().copied());

    match (fetch, detail) {
        (Some(fetch), Some(detail)) => format!("while fetching {fetch}: {detail}"),
        (_, Some(detail)) => detail.to_string(),
        _ => "unknown nix error".to_string(),
    }
}

fn summarize_missing_path_info(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);

    if stderr.contains("don't know how to build these paths:") {
        return "some store paths are not available locally or from substituters".to_string();
    }

    summarize_nix_stderr(stderr.as_bytes())
}

fn extract_between<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let rest = line.split_once(prefix)?.1;
    let value = rest.split_once(suffix)?.0.trim();
    (!value.is_empty()).then_some(value)
}

fn decode_nix_json<T>(stdout: &[u8], context: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    if let Ok(value) = serde_json::from_slice(stdout) {
        return Ok(value);
    }

    let stdout = String::from_utf8_lossy(stdout);
    let trimmed = stdout.trim();
    let start = trimmed.find(['{', '[']).ok_or_else(|| eyre!(context.to_string()))?;
    let end = trimmed
        .rfind(['}', ']'])
        .ok_or_else(|| eyre!(context.to_string()))?;
    let candidate = &trimmed[start..=end];

    serde_json::from_str(candidate).context(context.to_string())
}

#[cfg(test)]
mod tests {
    use std::hash::BuildHasherDefault;

    use indexmap::IndexMap;
    use rustc_hash::FxHasher;

    use super::{
        ArchivedFlake, collect_paths, decode_nix_json, format_bytes, summarize_missing_path_info,
        summarize_nix_stderr,
    };
    use crate::lock::{Input, Node, Resolve};

    #[test]
    fn keeps_direct_paths_when_a_follow_alias_is_missing_from_archive_json() {
        let lock = Resolve {
            root: Node {
                inputs: IndexMap::from_iter([
                    ("base".to_string(), Input::Direct("base".to_string())),
                    ("alias".to_string(), Input::Follow(vec!["base".to_string()])),
                ]),
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

        let archived = ArchivedFlake {
            path: "/nix/store/root-source".to_string(),
            inputs: IndexMap::from_iter([(
                "base".to_string(),
                ArchivedFlake {
                    path: "/nix/store/base-source".to_string(),
                    inputs: IndexMap::default(),
                },
            )]),
        };

        let mut by_node = std::collections::HashMap::new();
        collect_paths(&lock, &Input::Follow(Vec::new()), &archived, &mut by_node);

        assert_eq!(
            by_node.get(&crate::lock::NodeId::Node("base".to_string())),
            Some(&"/nix/store/base-source".to_string())
        );
        assert_eq!(by_node.len(), 1);
    }

    #[test]
    fn formats_human_sizes() {
        assert_eq!(format_bytes(None), "?");
        assert_eq!(format_bytes(Some(512)), "512 B");
        assert_eq!(format_bytes(Some(2048)), "2.0 KiB");
        assert_eq!(format_bytes(Some(5 * 1024 * 1024)), "5.0 MiB");
    }

    #[test]
    fn summarizes_nix_fetch_errors() {
        let stderr = br#"
unpacking 'github:ryantm/agenix/0000000000000000000000000000000000000000' into the Git cache...
error:
       ... while fetching the input 'github:ryantm/agenix/0000000000000000000000000000000000000000'

       error: Failed to open archive (Source threw exception: error: unable to download 'https://example.invalid/archive.tar.gz': HTTP error 404

              response body:

              404: Not Found)
"#;

        assert_eq!(
            summarize_nix_stderr(stderr),
            "while fetching github:ryantm/agenix/0000000000000000000000000000000000000000: Failed to open archive (Source threw exception: error: unable to download 'https://example.invalid/archive.tar.gz': HTTP error 404"
        );
    }

    #[test]
    fn decodes_json_with_prefixed_copy_progress() {
        let stdout = br#"
copying path '/nix/store/example-source' from 'https://cache.nixos.org'...
{"/nix/store/example-source":{"closureSize":42,"narSize":42},"/nix/store/missing-source":null}
"#;

        let decoded =
            decode_nix_json::<std::collections::HashMap<String, Option<super::PathInfo>>>(
                stdout,
                "failed to decode nix path-info output",
            )
            .unwrap();

        assert_eq!(
            decoded
                .get("/nix/store/example-source")
                .and_then(|info| info.as_ref())
                .and_then(|info| info.closure_size),
            Some(42)
        );
        assert!(decoded
            .get("/nix/store/missing-source")
            .is_some_and(|info| info.is_none()));
    }

    #[test]
    fn summarizes_missing_path_info_for_unknown_store_paths() {
        let stderr = br#"
these 2 paths will be fetched (0.00 MiB download, 0.00 MiB unpacked):
  /nix/store/example-source
don't know how to build these paths:
  /nix/store/missing-source
"#;

        assert_eq!(
            summarize_missing_path_info(stderr),
            "some store paths are not available locally or from substituters"
        );
    }
}
