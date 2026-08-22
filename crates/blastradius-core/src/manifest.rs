//! `workspace.yaml` — the single entry point (ADR-0004), and the include-glob
//! expansion. Globs support `*` (one segment) and `**` (any depth) only
//! (spec §1); anything fancier is a manifest error, not a silent no-match.

use crate::diagnostics::Diagnostic;
use crate::vfs::Vfs;
use crate::yaml;

pub struct Manifest {
    pub name: String,
    pub model_files: Vec<String>,
    pub view_files: Vec<String>,
    pub doc_files: Vec<String>,
}

pub fn load(vfs: &dyn Vfs, diags: &mut Vec<Diagnostic>) -> Option<Manifest> {
    let rel = "workspace.yaml";
    if vfs.read(rel).is_err() {
        diags.push(Diagnostic::error(rel, 0, "workspace.yaml not found — not a workspace"));
        return None;
    }
    let (node, _text) = yaml::load_file(vfs, rel, diags)?;
    let map = yaml::as_mapping(&node, rel, "workspace.yaml", diags)?;

    // workspace: { name, version }
    let ws = map.get_node("workspace").and_then(|n| match n {
        marked_yaml::Node::Mapping(m) => Some(m),
        _ => None,
    });
    let Some(ws) = ws else {
        diags.push(Diagnostic::error(rel, 1, "missing `workspace:` section"));
        return None;
    };
    let name = yaml::get_str(ws, "name").unwrap_or("Workspace").to_string();

    match yaml::get_str(ws, "version").map(|v| v.parse::<u64>()) {
        Some(Ok(v)) if v <= crate::SCHEMA_VERSION => {}
        Some(Ok(v)) => {
            // Version gate (spec §1): refuse with an upgrade message, never a partial parse.
            diags.push(Diagnostic::error(
                rel,
                yaml::field_line(ws, "version"),
                format!(
                    "workspace version {v} is newer than this build understands \
                     (max {}) — upgrade Blastradius",
                    crate::SCHEMA_VERSION
                ),
            ));
            return None;
        }
        _ => {
            diags.push(Diagnostic::error(
                rel,
                yaml::field_line(ws, "version"),
                "workspace.version must be an integer",
            ));
            return None;
        }
    }

    let mut section_globs = |section: &str| -> Vec<String> {
        let Some(marked_yaml::Node::Mapping(sec)) = map.get_node(section) else {
            return Vec::new();
        };
        let Some(inc) = sec.get_node("include") else {
            return Vec::new();
        };
        let Some(seq) = yaml::as_sequence(inc) else {
            diags.push(Diagnostic::error(
                rel,
                yaml::line_of(inc),
                format!("{section}.include must be a list of globs"),
            ));
            return Vec::new();
        };
        seq.iter().filter_map(yaml::as_str).map(str::to_string).collect()
    };

    let model_globs = section_globs("model");
    let view_globs = section_globs("views");
    let doc_globs = section_globs("docs");

    Some(Manifest {
        name,
        model_files: expand(vfs, &model_globs, rel, diags),
        view_files: expand(vfs, &view_globs, rel, diags),
        doc_files: expand(vfs, &doc_globs, rel, diags),
    })
}

/// Expand globs to sorted, deduplicated workspace-relative paths.
fn expand(vfs: &dyn Vfs, globs: &[String], rel: &str, diags: &mut Vec<Diagnostic>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in globs {
        if g.contains('\\') || g.starts_with('/') || g.split('/').any(|seg| seg == "..") {
            diags.push(Diagnostic::error(
                rel,
                0,
                format!("glob {g:?} must be relative with forward slashes"),
            ));
            continue;
        }
        let segments: Vec<&str> = g.split('/').collect();
        let mut matches = Vec::new();
        walk(vfs, "", &segments, &mut matches);
        matches.sort();
        for m in matches {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    out.sort();
    out
}

/// Recursive matcher: `*` matches within one segment, `**` any depth.
fn walk(vfs: &dyn Vfs, prefix: &str, segments: &[&str], out: &mut Vec<String>) {
    let Some((seg, rest)) = segments.split_first() else {
        return;
    };
    let names = vfs.list(prefix);
    let join = |name: &str| {
        if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") }
    };

    if *seg == "**" {
        // ** matches zero segments…
        walk(vfs, prefix, rest, out);
        // …or one-or-more: descend keeping the ** active.
        for (name, is_dir) in &names {
            if *is_dir {
                walk(vfs, &join(name), segments, out);
            }
        }
        return;
    }

    for (name, is_dir) in &names {
        if !segment_matches(seg, name) {
            continue;
        }
        let child = join(name);
        if rest.is_empty() {
            if !*is_dir {
                out.push(child);
            }
        } else if *is_dir {
            walk(vfs, &child, rest, out);
        }
    }
}

/// `*` within a single segment (no `?`, no character classes — spec §1).
fn segment_matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !name.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            return name.len() >= pos && name[pos..].ends_with(part);
        } else {
            match name[pos..].find(part) {
                Some(found) => pos += found + part.len(),
                None => return false,
            }
        }
    }
    true
}
