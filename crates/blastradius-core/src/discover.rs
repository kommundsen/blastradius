//! Workspace discovery: point Blastradius at a repo root and it finds the
//! workspace(s) inside. This is the ease-of-life half of the L4 story — the
//! app gets to know the *repo* root, not just the model folder, which is
//! the anchor future code-introspected elements will hang from.

use crate::manifest::{LEGACY_MANIFEST, MANIFEST};
use std::path::{Path, PathBuf};

/// How deep below the given folder we look. Real workspaces sit at
/// `docs/`-ish depths; anything deeper is a haystack, not a repo layout.
pub const MAX_DEPTH: usize = 4;

/// Directories never worth descending into: hidden dirs (`.git`, `.cache`)
/// and well-known dependency/build output.
fn skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "node_modules" | "target" | "dist" | "build" | "out" | "vendor"
        )
}

/// A cheap content sniff: is this file a Blastradius manifest? Looks for a
/// top-level `workspace:` key, which distinguishes our manifest both from
/// other tools' `workspace.yaml` files and from a *model* file that merely
/// shares the `blastradius.yaml` name (its top-level keys are element ids).
fn sniffs_as_manifest(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    text.lines().any(|l| l.starts_with("workspace:"))
}

/// Does this directory hold a workspace? Checks both manifest names, with
/// the content sniff so foreign files never count.
pub fn is_workspace_dir(dir: &Path) -> bool {
    [MANIFEST, LEGACY_MANIFEST]
        .iter()
        .any(|name| sniffs_as_manifest(&dir.join(name)))
}

/// Find workspace roots at or under `base`, breadth-first to [`MAX_DEPTH`].
/// If `base` itself is a workspace it is the only hit; a found workspace is
/// not searched for nested ones. Results are sorted, so callers behave
/// deterministically.
pub fn discover_workspaces(base: &Path) -> Vec<PathBuf> {
    if is_workspace_dir(base) {
        return vec![base.to_path_buf()];
    }
    let mut hits = Vec::new();
    let mut frontier = vec![base.to_path_buf()];
    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for dir in frontier {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() || skip_dir(&entry.file_name().to_string_lossy()) {
                    continue;
                }
                if is_workspace_dir(&path) {
                    hits.push(path);
                } else {
                    next.push(path);
                }
            }
        }
        frontier = next;
    }
    hits.sort();
    hits
}
