//! Cross-reference validation (spec §6) — everything that needs the whole
//! workspace in hand: reference resolution, duplicate relations, doc link
//! integrity, status vocabularies.

use crate::diagnostics::Diagnostic;
use crate::model::{ElementKind, Workspace};
use std::collections::{BTreeSet, HashMap};

/// Status vocabularies per doc type (spec §5).
fn allowed_statuses(doc_type: &str) -> Option<&'static [&'static str]> {
    match doc_type {
        "adr" => Some(&["proposed", "accepted", "superseded", "rejected"]),
        "prd" | "spec" | "roadmap" => Some(&["draft", "current", "superseded"]),
        "note" => Some(&[]),
        _ => None, // unknown type — warning below
    }
}

pub fn cross_validate(ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    // --- relations: resolve endpoints, catch verbatim duplicates -------------
    let mut seen: BTreeSet<(String, String, Option<String>)> = BTreeSet::new();
    for r in &ws.relations {
        let scope = r.scope.as_deref();
        for (end, reference) in [("from", &r.from), ("to", &r.to)] {
            if ws.resolve(reference, scope).is_none() {
                diags.push(Diagnostic::error(
                    &r.file as &str,
                    r.line,
                    format!("relation {end}: dangling reference {reference:?}"),
                ));
            }
        }
        let key = (r.from.clone(), r.to.clone(), r.label.clone());
        if !seen.insert(key) {
            diags.push(Diagnostic::warning(
                &r.file as &str,
                r.line,
                format!("relation {} -> {} duplicated verbatim", r.from, r.to),
            ));
        }
    }

    // --- container instances point at a real container (ADR-0018) -----------
    for el in ws.elements.values() {
        let Some(reference) = &el.instance_of else { continue };
        match ws.resolve(reference, None) {
            Some(target) if ws.elements[&target].kind == ElementKind::Container => {}
            Some(target) => diags.push(Diagnostic::error(
                &el.file as &str,
                el.line,
                format!(
                    "instance {:?}: `container: {reference}` resolves to a {}, not a container",
                    el.id,
                    ws.elements[&target].kind.as_str()
                ),
            )),
            None => diags.push(Diagnostic::error(
                &el.file as &str,
                el.line,
                format!("instance {:?}: dangling `container: {reference}`", el.id),
            )),
        }
    }

    // --- views: scope + pin targets ------------------------------------------
    for v in &ws.views {
        if !ws.elements.contains_key(&v.scope) {
            diags.push(Diagnostic::error(
                &v.file as &str,
                v.line,
                format!("view scope {:?} is not an element", v.scope),
            ));
            continue;
        }
        for pin in v.layout.keys() {
            if ws.resolve(pin, Some(&v.scope)).is_none() {
                diags.push(Diagnostic::error(
                    &v.file as &str,
                    v.line,
                    format!("layout pins unknown element {pin:?}"),
                ));
            }
        }
    }

    // --- docs: unique ids, vocab, element links, supersedes ------------------
    let mut doc_ids: HashMap<&str, &str> = HashMap::new(); // id -> file
    for d in &ws.docs {
        if let Some(first) = doc_ids.insert(&d.id, &d.file) {
            diags.push(Diagnostic::error(
                &d.file as &str,
                d.line,
                format!("duplicate doc id {:?} (already used in {first})", d.id),
            ));
        }
        match allowed_statuses(&d.doc_type) {
            None => diags.push(Diagnostic::warning(
                &d.file as &str,
                d.line,
                format!("unknown doc type {:?}", d.doc_type),
            )),
            Some([]) => {} // note: no status vocabulary
            Some(allowed) => match &d.status {
                Some(s) if allowed.contains(&s.as_str()) => {}
                Some(s) => diags.push(Diagnostic::error(
                    &d.file as &str,
                    d.line,
                    format!("status {s:?} invalid for type {:?} (allowed: {})", d.doc_type, allowed.join(" / ")),
                )),
                None => diags.push(Diagnostic::error(
                    &d.file as &str,
                    d.line,
                    format!("type {:?} requires a status ({})", d.doc_type, allowed.join(" / ")),
                )),
            },
        }
        for eid in &d.elements {
            if !ws.elements.contains_key(eid) {
                diags.push(Diagnostic::error(
                    &d.file as &str,
                    d.line,
                    format!("elements link dangling: {eid:?}"),
                ));
            }
        }
    }
    // supersedes targets — dangling is a warning (not in spec §6's error list)
    for d in &ws.docs {
        if let Some(target) = &d.supersedes {
            if !doc_ids.contains_key(target.as_str()) {
                diags.push(Diagnostic::warning(
                    &d.file as &str,
                    d.line,
                    format!("supersedes unknown doc {target:?}"),
                ));
            }
        }
    }
}
