//! Architecture drift detection (ADR-0019): does the code agree with the model?
//!
//! L4 extraction records references that leave a component's mapped corpus but
//! stay in the repository (`outbound`). Here those file paths are resolved to
//! whichever component's `source:` mapping owns them, which turns a raw
//! reference into a *code dependency between components* — the first fact in
//! this product that comes from reality rather than from the model describing
//! itself. Comparing that against the declared relations is the whole feature.
//!
//! Both findings are warnings. A team adopting this on an existing repo would
//! otherwise get a red build on day one; `--strict-drift` is how CI opts in.

use crate::diagnostics::Diagnostic;
use crate::model::{ElementId, Workspace};
use std::collections::{BTreeMap, BTreeSet};

/// A code dependency the model does not declare, or a declaration the code
/// does not back up.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Drift {
    pub from: ElementId,
    pub to: ElementId,
    pub kind: DriftKind,
    /// The file that evidences it (undeclared only).
    pub via: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriftKind {
    /// Code in `from` reaches into `to`, and nothing in the model says so.
    Undeclared,
    /// The model declares `from -> to`, both sides are introspected, and no
    /// code reference supports it.
    Unbacked,
}

/// Every element id and its ancestors — a relation between containers covers a
/// dependency between their components, which is how the canvas already lifts
/// edges. Without this, every intra-container dependency would look undeclared.
fn with_ancestors(id: &str) -> Vec<String> {
    let segs: Vec<&str> = id.split('.').collect();
    (1..=segs.len()).map(|n| segs[..n].join(".")).collect()
}

/// The component whose `source:` mapping owns a repo-relative file, if any.
/// A file no mapping claims is not drift — it is simply not introspected.
fn owner_of(ws: &Workspace, path: &str) -> Option<ElementId> {
    let mut best: Option<(usize, ElementId)> = None;
    for el in ws.elements.values() {
        let Some(m) = &el.source else { continue };
        let root = m.root.trim_end_matches('/');
        let Some(rest) = path.strip_prefix(root).and_then(|r| r.strip_prefix('/')) else { continue };
        if let Ok(Some(set)) = crate::introspect::glob_set(&m.include, "include") {
            if !set.is_match(rest) {
                continue;
            }
        }
        if let Ok(Some(set)) = crate::introspect::glob_set(&m.exclude, "exclude") {
            if set.is_match(rest) {
                continue;
            }
        }
        // Longest root wins, so nested mappings resolve to the inner one.
        if best.as_ref().is_none_or(|(len, _)| root.len() > *len) {
            best = Some((root.len(), el.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

/// Compare the code against the model. Returns findings in a stable order.
pub fn detect(ws: &Workspace) -> Vec<Drift> {
    let declared: BTreeSet<(String, String)> =
        ws.resolved_relations().into_iter().map(|(f, t, _)| (f, t)).collect();

    // A dependency is declared if any relation between the two elements *or
    // their ancestors* says so — a container-level relation covers what its
    // components do, which is the same lifting the canvas applies when it
    // draws an edge at a coarser altitude.
    let is_declared = |from: &str, to: &str| {
        with_ancestors(from).iter().any(|f| {
            with_ancestors(to).iter().any(|t| f != t && declared.contains(&(f.clone(), t.clone())))
        })
    };

    // Observed code dependencies between components, with one witness file.
    let mut observed: BTreeMap<(ElementId, ElementId), String> = BTreeMap::new();
    for g in &ws.derived {
        for (_, path) in &g.outbound {
            let Some(target) = owner_of(ws, path) else { continue };
            if target == g.component {
                continue; // a file this component owns but did not map in
            }
            observed.entry((g.component.clone(), target)).or_insert_with(|| path.clone());
        }
    }

    let mut out = Vec::new();
    for ((from, to), via) in &observed {
        if !is_declared(from, to) {
            out.push(Drift {
                from: from.clone(),
                to: to.clone(),
                kind: DriftKind::Undeclared,
                via: Some(via.clone()),
            });
        }
    }

    // The other direction only means something when an import *could* have
    // evidenced it: both sides introspected, and in the same language. A
    // TypeScript canvas talking to a Rust engine over IPC is a real relation
    // that no static import will ever show, so its silence proves nothing.
    let language: BTreeMap<&str, &str> =
        ws.derived.iter().map(|g| (g.component.as_str(), g.language.as_str())).collect();
    for (from, to, _) in ws.resolved_relations() {
        let (Some(lf), Some(lt)) = (language.get(from.as_str()), language.get(to.as_str())) else {
            continue;
        };
        if from == to || lf != lt {
            continue;
        }
        if !observed.contains_key(&(from.clone(), to.clone())) {
            out.push(Drift { from, to, kind: DriftKind::Unbacked, via: None });
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Drift as diagnostics, reported against the component that declares (or
/// fails to declare) the dependency.
pub fn diagnose(ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    for d in detect(ws) {
        let el = ws.elements.get(&d.from);
        let (file, line) = el.map(|e| (e.file.clone(), e.line)).unwrap_or_default();
        let message = match d.kind {
            DriftKind::Undeclared => format!(
                "drift: code in {:?} depends on {:?} (via {}), but no relation declares it",
                d.from,
                d.to,
                d.via.as_deref().unwrap_or("source")
            ),
            DriftKind::Unbacked => format!(
                "drift: {:?} -> {:?} is declared, but no code reference supports it",
                d.from, d.to
            ),
        };
        diags.push(Diagnostic::warning(&file as &str, line, message));
    }
}
