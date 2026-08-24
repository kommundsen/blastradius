//! In-app merge conflict resolution (0.2.0 theme 3, ADR-0015): per-element
//! ours/theirs decisions, applied as CST splices onto the chosen base
//! side's stage text — comments and formatting of the kept side survive,
//! exactly like every other write in the app. The result is validated
//! before anything touches disk, then written and staged through the
//! user's *own* `git` binary: libgit2 stays read-only (ADR-0007), the same
//! division of labour `blastradius init` uses for `git init`.

use crate::diff;
use crate::git::GitContext;
use crate::model::ElementKind;
use crate::splice;
use crate::vfs::{DiskVfs, OverlayVfs};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Ours,
    Theirs,
}

/// The user's decisions. Everything undecided keeps ours (git's convention
/// for "mine wins by default" — and the side whose comments survive).
#[derive(Deserialize, Debug, Default)]
pub struct Resolution {
    /// Per-file base side; also the whole answer for files with no
    /// element-level conflicts (views, docs).
    #[serde(default)]
    pub files: HashMap<String, Side>,
    /// Per-element choice, by full element id.
    #[serde(default)]
    pub elements: HashMap<String, Side>,
}

/// Resolve the current merge conflict. Returns the workspace-relative files
/// that were written (or deleted) and staged.
pub fn resolve(
    ctx: &GitContext,
    workspace_root: &Path,
    res: &Resolution,
) -> Result<Vec<String>, String> {
    let Some((files, ours_over, theirs_over)) = ctx.stage_overrides()? else {
        return Err("no merge conflict to resolve".to_string());
    };

    let disk = DiskVfs::new(workspace_root);
    let (ours_ws, _) =
        crate::load_workspace_vfs(&OverlayVfs { base: &disk, overrides: ours_over.clone() });
    let (theirs_ws, _) =
        crate::load_workspace_vfs(&OverlayVfs { base: &disk, overrides: theirs_over.clone() });
    let conflicted = diff::diff(&ours_ws, &theirs_ws);

    // Build each conflicted file's resolved text.
    let mut resolved: HashMap<String, Option<String>> = HashMap::new(); // None = delete
    for rel in &files {
        let base_side = res.files.get(rel).copied().unwrap_or(Side::Ours);
        let (base_over, other_ws) = match base_side {
            Side::Ours => (&ours_over, &theirs_ws),
            Side::Theirs => (&theirs_over, &ours_ws),
        };
        let base_ws = match base_side {
            Side::Ours => &ours_ws,
            Side::Theirs => &theirs_ws,
        };
        let Some(mut text) = base_over.get(rel).cloned() else {
            // the chosen side deleted this file — the resolution is deletion
            resolved.insert(rel.clone(), None);
            continue;
        };

        // Splice in every element of *this file* decided against the base.
        for id in conflicted.elements.keys() {
            let choice = res.elements.get(id).copied().unwrap_or(Side::Ours);
            if choice == base_side {
                continue;
            }
            let in_base = base_ws.elements.get(id);
            let in_other = other_ws.elements.get(id);
            let file_of = in_base.or(in_other).map(|e| e.file.as_str());
            if file_of != Some(rel.as_str()) {
                continue;
            }
            let chain_owner = in_base.or(in_other).unwrap();
            let chain = element_chain(chain_owner);
            let chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
            match (in_base, in_other) {
                (Some(b), Some(o)) => {
                    // field-level: take the chosen side's values where they differ
                    if b.name != o.name {
                        text = splice::set_field(&text, &chain_refs, "name", &o.name)?;
                    }
                    for (field, bv, ov) in
                        [("tech", &b.tech, &o.tech), ("description", &b.description, &o.description)]
                    {
                        match (bv, ov) {
                            (_, Some(v)) if bv != ov => {
                                text = splice::set_field(&text, &chain_refs, field, v)?;
                            }
                            (Some(_), None) => {
                                let mut field_chain = chain_refs.clone();
                                field_chain.push(field);
                                text = splice::remove_entry(&text, &field_chain)?;
                            }
                            _ => {}
                        }
                    }
                }
                (Some(b), None) => {
                    // chosen side deleted it
                    if b.kind == ElementKind::System {
                        return Err(format!(
                            "{id}: a whole system differs — resolve {rel} by file (ours/theirs), not per element"
                        ));
                    }
                    text = splice::remove_entry(&text, &chain_refs)?;
                }
                (None, Some(o)) => {
                    // chosen side added it — insert with its fields
                    if o.kind == ElementKind::System {
                        return Err(format!(
                            "{id}: a whole system differs — resolve {rel} by file (ours/theirs), not per element"
                        ));
                    }
                    let (section, id_key) = chain.split_at(chain.len() - 1);
                    let section_refs: Vec<&str> = section.iter().map(String::as_str).collect();
                    let owner: Vec<&str> =
                        section_refs.iter().take(section_refs.len().saturating_sub(1)).copied().collect();
                    let mut fields: Vec<(&str, &str)> = vec![("name", o.name.as_str())];
                    if let Some(t) = &o.tech {
                        fields.push(("tech", t));
                    }
                    if let Some(d) = &o.description {
                        fields.push(("description", d));
                    }
                    let indent = (section_refs.len().saturating_sub(1)) * 2;
                    text = splice::insert_entry(
                        &text,
                        &section_refs,
                        Some((&owner, indent)),
                        &id_key[0],
                        &fields,
                    )?;
                }
                (None, None) => {}
            }
        }
        resolved.insert(rel.clone(), Some(text));
    }

    // Validate the whole workspace with the resolutions overlaid — a
    // resolution that produces an invalid model is refused outright.
    let overrides: HashMap<String, String> = resolved
        .iter()
        .filter_map(|(k, v)| v.clone().map(|t| (k.clone(), t)))
        .collect();
    let (_, diags) = crate::load_workspace_vfs(&OverlayVfs { base: &disk, overrides });
    if crate::diagnostics::has_errors(&diags) {
        let first = diags
            .iter()
            .find(|d| d.severity == crate::diagnostics::Severity::Error)
            .map(|d| d.to_string())
            .unwrap_or_default();
        return Err(format!("resolution would break the workspace: {first}"));
    }

    // Write, then stage through the user's own git binary (ADR-0007: our
    // libgit2 stays read-only; staging is the user's tooling acting for them).
    for (rel, text) in &resolved {
        let path = workspace_root.join(rel);
        match text {
            Some(t) => std::fs::write(&path, t).map_err(|e| format!("{rel}: {e}"))?,
            None => {
                if path.exists() {
                    std::fs::remove_file(&path).map_err(|e| format!("{rel}: {e}"))?;
                }
            }
        }
    }
    let workdir = ctx.workdir().ok_or("repository has no working directory")?;
    let repo_paths: Vec<String> = files.iter().map(|f| ctx.to_repo_path(f)).collect();
    let out = std::process::Command::new("git")
        .arg("add")
        .arg("--")
        .args(&repo_paths)
        .current_dir(&workdir)
        .output()
        .map_err(|e| format!("files written, but `git add` failed to run: {e} — stage them yourself"))?;
    if !out.status.success() {
        return Err(format!(
            "files written, but staging failed: {} — run `git add` yourself",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let mut written: Vec<String> = resolved.keys().cloned().collect();
    written.sort();
    Ok(written)
}

/// Key chain addressing an element's mapping in its file (mirrors the sync
/// engine's addressing — spec §3 layout).
fn element_chain(el: &crate::model::Element) -> Vec<String> {
    let segs: Vec<&str> = el.id.split('.').collect();
    match el.kind {
        ElementKind::Person => vec!["people".into(), el.id.clone()],
        ElementKind::External => vec!["external".into(), el.id.clone()],
        ElementKind::System => vec![],
        ElementKind::Container => vec!["containers".into(), segs[1].into()],
        ElementKind::Component => vec![
            "containers".into(),
            segs[1].into(),
            "components".into(),
            segs[2].into(),
        ],
        k if k.is_deployment() => crate::model::deployment_chain(&el.id, k),
        _ => unreachable!("every kind is addressed above"),
    }
}
