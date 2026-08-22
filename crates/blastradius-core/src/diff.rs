//! Semantic model diff (ADR-0007, spec/git-and-diff.md): compare two parsed
//! workspaces as element graphs. Same id = same element (ADR-0003), so a
//! rename is a `Changed` name field, never a remove+add pair.
//!
//! Layout (views) is deliberately outside the diff — a moved box is not an
//! architecture change. Doc-link changes surface as `Changed` on the linked
//! elements.

use crate::model::{Relation, Workspace};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Default)]
pub struct ModelDiff {
    /// element id -> change
    pub elements: BTreeMap<String, Change>,
    /// (from, to, label) -> change
    pub relations: BTreeMap<(String, String, String), Change>,
}

impl ModelDiff {
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty() && self.relations.is_empty()
    }
}

fn relation_key(r: &Relation) -> (String, String, String) {
    (r.from.clone(), r.to.clone(), r.label.clone().unwrap_or_default())
}

/// Fields that make an element `Changed` (spec/git-and-diff.md): name, tech,
/// description, kind, external flag.
fn element_fingerprint(ws: &Workspace, id: &str) -> Option<String> {
    let e = ws.elements.get(id)?;
    Some(format!(
        "{}|{}|{}|{}|{}",
        e.kind.as_str(),
        e.name,
        e.tech.as_deref().unwrap_or(""),
        e.description.as_deref().unwrap_or(""),
        e.external
    ))
}

/// The set of doc ids linked to an element — link changes count as Changed.
fn doc_links<'a>(ws: &'a Workspace, id: &str) -> BTreeSet<&'a str> {
    ws.docs
        .iter()
        .filter(|d| d.elements.iter().any(|e| e == id))
        .map(|d| d.id.as_str())
        .collect()
}

pub fn diff(base: &Workspace, current: &Workspace) -> ModelDiff {
    let mut out = ModelDiff::default();

    let base_ids: BTreeSet<&String> = base.elements.keys().collect();
    let cur_ids: BTreeSet<&String> = current.elements.keys().collect();

    for id in cur_ids.difference(&base_ids) {
        out.elements.insert((*id).clone(), Change::Added);
    }
    for id in base_ids.difference(&cur_ids) {
        out.elements.insert((*id).clone(), Change::Removed);
    }
    for id in base_ids.intersection(&cur_ids) {
        let same_fields = element_fingerprint(base, id) == element_fingerprint(current, id);
        let same_docs = doc_links(base, id) == doc_links(current, id);
        if !(same_fields && same_docs) {
            out.elements.insert((*id).clone(), Change::Changed);
        }
    }

    // Relations: keyed by (from, to, label); protocol/direction edits = Changed.
    let base_rels: BTreeMap<_, &Relation> =
        base.relations.iter().map(|r| (relation_key(r), r)).collect();
    let cur_rels: BTreeMap<_, &Relation> =
        current.relations.iter().map(|r| (relation_key(r), r)).collect();

    for (key, cur) in &cur_rels {
        match base_rels.get(key) {
            None => {
                out.relations.insert(key.clone(), Change::Added);
            }
            Some(b) => {
                if b.protocol != cur.protocol || b.direction != cur.direction {
                    out.relations.insert(key.clone(), Change::Changed);
                }
            }
        }
    }
    for key in base_rels.keys() {
        if !cur_rels.contains_key(key) {
            out.relations.insert(key.clone(), Change::Removed);
        }
    }

    out
}

// ---- renderer payload -------------------------------------------------------

#[derive(Serialize)]
pub struct DiffPayload {
    /// The base revision this diff is against (short id or refspec).
    pub base: String,
    pub elements: Vec<DiffElement>,
    pub relations: Vec<DiffRelation>,
    /// Views whose pinned layout differs, with the moved/added/removed pin ids
    /// — excluded from the semantic diff proper (spec/git-and-diff.md), shown
    /// only behind the layout toggle.
    pub layout: Vec<LayoutChange>,
}

#[derive(Serialize)]
pub struct DiffElement {
    pub id: String,
    /// added | removed | changed
    pub change: &'static str,
    /// Element data — from the current model when present, else from base
    /// (removed elements need this to render as ghosts).
    pub element: crate::snapshot::SnapElement,
}

#[derive(Serialize)]
pub struct DiffRelation {
    /// Resolved endpoints, renderer-ready.
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub change: &'static str,
}

#[derive(Serialize)]
pub struct LayoutChange {
    pub view: String,
    pub pins: Vec<String>,
}

fn change_str(c: Change) -> &'static str {
    match c {
        Change::Added => "added",
        Change::Removed => "removed",
        Change::Changed => "changed",
    }
}

/// The semantic diff in renderer form. Both workspaces must be valid.
pub fn diff_payload(base_label: &str, base: &Workspace, current: &Workspace) -> DiffPayload {
    let d = diff(base, current);

    let snap_el = |ws: &Workspace, id: &str| -> Option<crate::snapshot::SnapElement> {
        ws.elements.get(id).map(crate::snapshot::snap_element)
    };

    let elements = d
        .elements
        .iter()
        .filter_map(|(id, change)| {
            let element = snap_el(current, id).or_else(|| snap_el(base, id))?;
            Some(DiffElement { id: id.clone(), change: change_str(*change), element })
        })
        .collect();

    // Resolve relation endpoints against whichever side knows them.
    let resolve = |raw: &str, scope: Option<&str>| {
        current
            .resolve(raw, scope)
            .or_else(|| base.resolve(raw, scope))
            .unwrap_or_else(|| raw.to_string())
    };
    let scope_of = |ws: &Workspace, key: &(String, String, String)| {
        ws.relations
            .iter()
            .find(|r| {
                r.from == key.0 && r.to == key.1 && r.label.clone().unwrap_or_default() == key.2
            })
            .and_then(|r| r.scope.clone())
    };
    let relations = d
        .relations
        .iter()
        .map(|(key, change)| {
            let scope = scope_of(current, key).or_else(|| scope_of(base, key));
            DiffRelation {
                from: resolve(&key.0, scope.as_deref()),
                to: resolve(&key.1, scope.as_deref()),
                label: (!key.2.is_empty()).then(|| key.2.clone()),
                change: change_str(*change),
            }
        })
        .collect();

    // Pinned-layout differences per view id.
    let mut layout = Vec::new();
    let base_views: BTreeMap<&str, _> = base.views.iter().map(|v| (v.id.as_str(), v)).collect();
    for v in &current.views {
        let base_pins = base_views.get(v.id.as_str()).map(|b| &b.layout);
        let mut pins: Vec<String> = Vec::new();
        for (id, xy) in &v.layout {
            if base_pins.and_then(|b| b.get(id)) != Some(xy) {
                pins.push(id.clone());
            }
        }
        if let Some(bp) = base_pins {
            for id in bp.keys() {
                if !v.layout.contains_key(id) {
                    pins.push(id.clone());
                }
            }
        }
        if !pins.is_empty() {
            pins.sort();
            layout.push(LayoutChange { view: v.id.clone(), pins });
        }
    }
    for (id, b) in &base_views {
        if !current.views.iter().any(|v| v.id == *id) && !b.layout.is_empty() {
            layout.push(LayoutChange {
                view: id.to_string(),
                pins: b.layout.keys().cloned().collect(),
            });
        }
    }

    DiffPayload { base: base_label.to_string(), elements, relations, layout }
}
