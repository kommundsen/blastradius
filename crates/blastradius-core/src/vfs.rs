//! File-source abstraction. The same parser must load a workspace from the
//! working tree, from a git revision (spec/git-and-diff.md: materialise blobs
//! in-memory, no checkout), or from a conflict stage — so nothing below the
//! workspace loader touches std::fs directly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub trait Vfs {
    /// Read a workspace-relative file (forward-slash path).
    fn read(&self, rel: &str) -> Result<String, String>;
    /// List a workspace-relative directory: (name, is_dir), sorted by name.
    /// Missing directory = empty.
    fn list(&self, dir: &str) -> Vec<(String, bool)>;
}

/// The working tree.
pub struct DiskVfs {
    root: PathBuf,
}

impl DiskVfs {
    pub fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }
}

impl Vfs for DiskVfs {
    fn read(&self, rel: &str) -> Result<String, String> {
        std::fs::read_to_string(self.root.join(rel)).map_err(|e| e.to_string())
    }

    fn list(&self, dir: &str) -> Vec<(String, bool)> {
        let path = if dir.is_empty() { self.root.clone() } else { self.root.join(dir) };
        let Ok(entries) = std::fs::read_dir(path) else {
            return Vec::new();
        };
        let mut out: Vec<(String, bool)> = entries
            .flatten()
            .filter_map(|e| {
                Some((e.file_name().to_string_lossy().into_owned(), e.file_type().ok()?.is_dir()))
            })
            .collect();
        out.sort();
        out
    }
}

/// A base source with in-memory replacements — how a conflict side is built:
/// the working tree, with each conflicted file's content swapped for its
/// stage-2 (ours) or stage-3 (theirs) blob.
pub struct OverlayVfs<'a> {
    pub base: &'a dyn Vfs,
    pub overrides: HashMap<String, String>,
}

impl Vfs for OverlayVfs<'_> {
    fn read(&self, rel: &str) -> Result<String, String> {
        match self.overrides.get(rel) {
            Some(text) => Ok(text.clone()),
            None => self.base.read(rel),
        }
    }

    fn list(&self, dir: &str) -> Vec<(String, bool)> {
        self.base.list(dir)
    }
}
