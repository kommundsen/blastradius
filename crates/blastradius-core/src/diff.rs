//! Semantic model diff (ADR-0007, spec/git-and-diff.md): compare two parsed
//! workspaces as element graphs. Same id = same element (ADR-0003), so a
//! rename is a `Changed` name field, never a remove+add pair.
//!
//! Layout (views) is deliberately outside the diff — a moved box is not an
//! architecture change. Doc-link changes surface as `Changed` on the linked
//! elements.

use crate::model::{Relation, Workspace};
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
