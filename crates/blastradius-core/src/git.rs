//! Git service (ADR-0007): embedded libgit2, strictly read-only. Status,
//! revision materialisation for semantic diff, conflict stages, history.
//! The app performs no commits, merges, or pushes — the user's own git
//! tooling owns writes.

use crate::diagnostics::Diagnostic;
use crate::model::Workspace;
use crate::vfs::{OverlayVfs, Vfs};
use git2::{Repository, Sort};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub struct GitContext {
    repo: Repository,
    /// Workspace root relative to the repo workdir, forward-slash, "" = same.
    prefix: String,
}

#[derive(Serialize, Debug)]
pub struct GitStatus {
    pub branch: String,
    /// Changed workspace files (worktree vs HEAD), conflict files included.
    pub dirty: usize,
    pub ahead: usize,
    pub behind: usize,
    /// Workspace-relative conflicted files.
    pub conflicted: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct CommitInfo {
    pub id: String,
    pub short: String,
    pub summary: String,
    pub author: String,
    /// Seconds since epoch.
    pub time: i64,
}

/// One element's field values on one side of a conflict.
#[derive(Serialize, Debug)]
pub struct ConflictSide {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ConflictElement {
    pub id: String,
    /// Absent side = the element does not exist on that side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<ConflictSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<ConflictSide>,
}

#[derive(Serialize, Debug)]
pub struct Conflicts {
    pub files: Vec<String>,
    pub elements: Vec<ConflictElement>,
}

impl GitContext {
    /// Discover the repository containing the workspace. None = no repo, and
    /// every git surface is absent, not an error (ADR-0007).
    pub fn discover(workspace_root: &Path) -> Option<GitContext> {
        let repo = Repository::discover(workspace_root).ok()?;
        let workdir = repo.workdir()?.to_path_buf();
        let canonical_root = workspace_root.canonicalize().ok()?;
        let canonical_workdir = workdir.canonicalize().ok()?;
        let prefix = canonical_root
            .strip_prefix(&canonical_workdir)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        Some(GitContext { repo, prefix })
    }

    /// Workspace-relative path -> repo-relative (for staging).
    pub(crate) fn to_repo_path(&self, ws_rel: &str) -> String {
        if self.prefix.is_empty() {
            ws_rel.to_string()
        } else {
            format!("{}/{}", self.prefix, ws_rel)
        }
    }

    /// The repository's working directory (bare repos have none).
    pub(crate) fn workdir(&self) -> Option<std::path::PathBuf> {
        self.repo.workdir().map(|p| p.to_path_buf())
    }

    /// Re-read the index and report whether conflicts remain.
    pub fn has_conflicts(&self) -> bool {
        self.repo
            .index()
            .ok()
            .and_then(|mut i| i.read(false).ok().map(|()| i.has_conflicts()))
            .unwrap_or(false)
    }

    /// Repo path -> workspace-relative, when inside the workspace.
    fn from_repo_path(&self, repo_rel: &str) -> Option<String> {
        if self.prefix.is_empty() {
            return Some(repo_rel.to_string());
        }
        repo_rel
            .strip_prefix(&format!("{}/", self.prefix))
            .map(str::to_string)
    }

    pub fn status(&self) -> Result<GitStatus, String> {
        let head = self.repo.head().map_err(|e| e.to_string())?;
        let branch = head.shorthand().unwrap_or("HEAD").to_string();

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        if !self.prefix.is_empty() {
            opts.pathspec(&self.prefix);
        }
        let statuses = self.repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;
        let mut dirty = 0usize;
        let mut conflicted = Vec::new();
        for entry in statuses.iter() {
            let st = entry.status();
            if st.is_ignored() {
                continue;
            }
            let Some(path) = entry.path() else { continue };
            let Some(ws_rel) = self.from_repo_path(path) else { continue };
            if st.is_conflicted() {
                conflicted.push(ws_rel);
                dirty += 1;
            } else if !st.is_empty() {
                dirty += 1;
            }
        }
        conflicted.sort();

        let (ahead, behind) = self.ahead_behind().unwrap_or((0, 0));
        Ok(GitStatus { branch, dirty, ahead, behind, conflicted })
    }

    fn ahead_behind(&self) -> Option<(usize, usize)> {
        let head = self.repo.head().ok()?;
        let local = head.target()?;
        let branch = git2::Branch::wrap(head);
        let upstream = branch.upstream().ok()?;
        let upstream_oid = upstream.get().target()?;
        self.repo.graph_ahead_behind(local, upstream_oid).ok()
    }

    /// Load the workspace as it was at a revision — blobs in memory, no
    /// checkout (spec/git-and-diff.md).
    pub fn load_at(&self, refspec: &str) -> Result<(Workspace, Vec<Diagnostic>), String> {
        let vfs = self.tree_vfs(refspec)?;
        Ok(crate::load_workspace_vfs(&vfs))
    }

    /// A snapshot of the workspace at a revision, for time-travel rendering.
    pub fn snapshot_at(&self, refspec: &str) -> Result<crate::snapshot::Snapshot, String> {
        let vfs = self.tree_vfs(refspec)?;
        let (ws, diags) = crate::load_workspace_vfs(&vfs);
        Ok(crate::snapshot::snapshot(&vfs, &ws, &diags))
    }

    fn tree_vfs(&self, refspec: &str) -> Result<GitTreeVfs<'_>, String> {
        let obj = self.repo.revparse_single(refspec).map_err(|e| e.to_string())?;
        let commit = obj.peel_to_commit().map_err(|e| e.to_string())?;
        let tree = commit.tree().map_err(|e| e.to_string())?;
        Ok(GitTreeVfs { repo: &self.repo, tree, prefix: self.prefix.clone() })
    }

    /// Merge-base of HEAD with the default branch — the spec's default diff
    /// base. On the default branch itself this is HEAD (empty diff).
    pub fn default_base(&self) -> Option<String> {
        let head = self.repo.head().ok()?.target()?;
        let default = ["refs/remotes/origin/HEAD", "refs/remotes/origin/main",
                       "refs/remotes/origin/master", "refs/heads/main", "refs/heads/master"]
            .iter()
            .find_map(|r| self.repo.find_reference(r).ok()?.target());
        let base = self.repo.merge_base(head, default?).ok()?;
        Some(base.to_string())
    }

    /// Commits touching workspace files, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<CommitInfo>, String> {
        let mut walk = self.repo.revwalk().map_err(|e| e.to_string())?;
        walk.push_head().map_err(|e| e.to_string())?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME).map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for oid in walk.flatten() {
            if out.len() >= limit {
                break;
            }
            let Ok(commit) = self.repo.find_commit(oid) else { continue };
            if !self.touches_workspace(&commit) {
                continue;
            }
            out.push(CommitInfo {
                id: oid.to_string(),
                short: oid.to_string()[..8].to_string(),
                summary: commit.summary().unwrap_or("").to_string(),
                author: commit.author().name().unwrap_or("").to_string(),
                time: commit.time().seconds(),
            });
        }
        Ok(out)
    }

    fn touches_workspace(&self, commit: &git2::Commit) -> bool {
        let Ok(tree) = commit.tree() else { return false };
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let mut opts = git2::DiffOptions::new();
        if !self.prefix.is_empty() {
            opts.pathspec(&self.prefix);
        }
        match self.repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts)) {
            Ok(diff) => diff.deltas().len() > 0,
            Err(_) => false,
        }
    }

    /// While merge conflicts exist, the on-disk files carry conflict markers
    /// and do not parse — but the model must stay viewable (spec). Renders the
    /// OURS side: stage-2 blobs overlaid on the working tree. None when clean.
    pub fn ours_snapshot(
        &self,
        workspace_root: &Path,
    ) -> Result<Option<crate::snapshot::Snapshot>, String> {
        let Some((_files, ours_over, _theirs)) = self.stage_overrides()? else {
            return Ok(None);
        };
        let disk = crate::vfs::DiskVfs::new(workspace_root);
        let overlay = OverlayVfs { base: &disk, overrides: ours_over };
        let (ws, diags) = crate::load_workspace_vfs(&overlay);
        Ok(Some(crate::snapshot::snapshot(&overlay, &ws, &diags)))
    }

    /// Conflicted files + stage-2/stage-3 content maps; None when the index
    /// has no conflicts. Always re-reads the index — external tools resolve
    /// conflicts out from under us.
    #[allow(clippy::type_complexity)]
    pub(crate) fn stage_overrides(
        &self,
    ) -> Result<Option<(Vec<String>, HashMap<String, String>, HashMap<String, String>)>, String>
    {
        let mut index = self.repo.index().map_err(|e| e.to_string())?;
        index.read(false).map_err(|e| e.to_string())?;
        if !index.has_conflicts() {
            return Ok(None);
        }
        let mut files = Vec::new();
        let mut ours_over = HashMap::new();
        let mut theirs_over = HashMap::new();
        for c in index.conflicts().map_err(|e| e.to_string())?.flatten() {
            let path = c
                .our
                .as_ref()
                .or(c.their.as_ref())
                .or(c.ancestor.as_ref())
                .map(|e| String::from_utf8_lossy(&e.path).into_owned());
            let Some(repo_rel) = path else { continue };
            let Some(ws_rel) = self.from_repo_path(&repo_rel) else { continue };
            files.push(ws_rel.clone());
            for (entry, over) in [(&c.our, &mut ours_over), (&c.their, &mut theirs_over)] {
                if let Some(e) = entry {
                    if let Ok(blob) = self.repo.find_blob(e.id) {
                        over.insert(
                            ws_rel.clone(),
                            String::from_utf8_lossy(blob.content()).into_owned(),
                        );
                    }
                }
            }
        }
        files.sort();
        files.dedup();
        if files.is_empty() {
            return Ok(None);
        }
        Ok(Some((files, ours_over, theirs_over)))
    }

    /// During a merge conflict: both sides of every conflicted element, built
    /// by overlaying stage-2 (ours) / stage-3 (theirs) blob content onto the
    /// working tree and parsing each side with the ordinary loader.
    pub fn conflicts(&self, workspace_root: &Path) -> Result<Option<Conflicts>, String> {
        let Some((files, ours_over, theirs_over)) = self.stage_overrides()? else {
            return Ok(None);
        };

        let disk = crate::vfs::DiskVfs::new(workspace_root);
        let (ours_ws, _) =
            crate::load_workspace_vfs(&OverlayVfs { base: &disk, overrides: ours_over });
        let (theirs_ws, _) =
            crate::load_workspace_vfs(&OverlayVfs { base: &disk, overrides: theirs_over });

        // Elements differing between the sides are the conflicted set.
        let d = crate::diff::diff(&ours_ws, &theirs_ws);
        let side = |ws: &Workspace, id: &str| {
            ws.elements.get(id).map(|e| ConflictSide {
                name: e.name.clone(),
                tech: e.tech.clone(),
                description: e.description.clone(),
            })
        };
        let elements = d
            .elements
            .keys()
            .map(|id| ConflictElement {
                id: id.clone(),
                ours: side(&ours_ws, id),
                theirs: side(&theirs_ws, id),
            })
            .collect();

        Ok(Some(Conflicts { files, elements }))
    }
}

/// Read-only view of a workspace subtree inside a git tree.
pub struct GitTreeVfs<'r> {
    repo: &'r Repository,
    tree: git2::Tree<'r>,
    prefix: String,
}

impl GitTreeVfs<'_> {
    fn entry_path(&self, rel: &str) -> String {
        if self.prefix.is_empty() {
            rel.to_string()
        } else if rel.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{}", self.prefix, rel)
        }
    }
}

impl Vfs for GitTreeVfs<'_> {
    fn read(&self, rel: &str) -> Result<String, String> {
        let entry = self
            .tree
            .get_path(Path::new(&self.entry_path(rel)))
            .map_err(|e| e.to_string())?;
        let blob = self
            .repo
            .find_blob(entry.id())
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(blob.content()).into_owned())
    }

    fn list(&self, dir: &str) -> Vec<(String, bool)> {
        let path = self.entry_path(dir);
        let subtree = if path.is_empty() {
            Some(self.tree.clone())
        } else {
            self.tree
                .get_path(Path::new(&path))
                .ok()
                .and_then(|e| e.to_object(self.repo).ok())
                .and_then(|o| o.into_tree().ok())
        };
        let Some(subtree) = subtree else {
            return Vec::new();
        };
        let mut out: Vec<(String, bool)> = subtree
            .iter()
            .filter_map(|e| {
                Some((e.name()?.to_string(), e.kind() == Some(git2::ObjectType::Tree)))
            })
            .collect();
        out.sort();
        out
    }
}
