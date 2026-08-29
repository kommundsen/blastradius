//! The sealed snapshot: everything a rendering surface needs, as one JSON
//! value. Consumed by the Tauri WebView (Phase 1), the HTML export
//! (spec/export.md — the same shape becomes the v2 hosted-link payload), and
//! test harnesses.
//!
//! Read-only by construction: the snapshot carries no file paths the renderer
//! could write back to — surfaces propose edits through the sync engine only
//! (ADR-0008), never by mutating snapshot data.

use crate::diagnostics::Diagnostic;
use crate::model::{Direction, ElementKind, Workspace};
use crate::vfs::Vfs;
use serde::Serialize;

#[derive(Serialize)]
pub struct Snapshot {
    pub name: String,
    pub elements: Vec<SnapElement>,
    pub relations: Vec<SnapRelation>,
    pub views: Vec<SnapView>,
    pub docs: Vec<SnapDoc>,
    pub diagnostics: Vec<SnapDiagnostic>,
    /// Source-derived L4 graphs, one per opted-in component
    /// (spec/l4-introspection.md). Read-only by nature, not just by contract.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub derived: Vec<SnapDerived>,
    /// Where the code and the model disagree (ADR-0019). `drift::diagnose`
    /// flattens the same findings into warning strings; a renderer needs them
    /// whole, because the remedy for an undeclared dependency is one operation
    /// and a warning string cannot carry it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub drift: Vec<SnapDrift>,
}

/// One disagreement between the code and the model.
#[derive(Serialize)]
pub struct SnapDrift {
    pub from: String,
    pub to: String,
    /// undeclared | unbacked
    pub kind: &'static str,
    /// The repo-relative file that evidences it — undeclared findings only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

#[derive(Serialize)]
pub struct SnapDerived {
    pub component: String,
    pub language: String,
    pub stale: bool,
    pub elements: Vec<SnapDerivedElement>,
    pub edges: Vec<SnapDerivedEdge>,
}

#[derive(Serialize)]
pub struct SnapDerivedElement {
    pub id: String,
    /// module | namespace | class | interface | record | enum
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

#[derive(Serialize)]
pub struct SnapDerivedEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct SnapElement {
    pub id: String,
    /// person | external | system | container | component
    pub kind: &'static str,
    /// Parent dotted id, absent for roots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
    /// Visual grouping label (spec §3c) — omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// How many of this run (ADR-0018) — deployment only, omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    /// The L4 opt-in (spec/l4-introspection.md). Carried so an editing surface
    /// can show and change it; the facts it governs ride in `derived`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SnapSource>,
}

/// A component's `source:` mapping, as the renderer sees it.
#[derive(Serialize)]
pub struct SnapSource {
    pub language: String,
    pub root: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,
}

#[derive(Serialize)]
pub struct SnapRelation {
    /// Resolved dotted ids (unresolvable endpoints stay raw; the workspace is
    /// invalid in that case and the renderer shows diagnostics instead).
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// forward | both | none
    pub direction: &'static str,
}

#[derive(Serialize)]
pub struct SnapView {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub scope: String,
    pub level: String,
    /// element id -> [x, y] grid units (pins only — auto-layout is the renderer's job)
    pub layout: std::collections::BTreeMap<String, (f64, f64)>,
    pub include_context: bool,
    /// Draw `group:` boundaries in this view (spec §3c); off by default.
    pub show_groups: bool,
    /// Elements whose description is drawn in their box (spec §4), as written
    /// in the view file — scope-relative, resolved by the renderer like a pin.
    pub descriptions: Vec<String>,
    /// Draw a deployment view as boxes inside boxes (ADR-0018); off by default.
    pub nested: bool,
}

#[derive(Serialize)]
pub struct SnapDoc {
    pub id: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub elements: Vec<String>,
    pub file: String,
    /// First `# heading` of the body, else the id.
    pub title: String,
    /// Markdown body with frontmatter stripped.
    pub body: String,
}

#[derive(Serialize)]
pub struct SnapDiagnostic {
    pub severity: String,
    pub file: String,
    pub line: u64,
    pub message: String,
}

fn kind_str(k: ElementKind) -> &'static str {
    match k {
        ElementKind::Person => "person",
        ElementKind::External => "external",
        ElementKind::System => "system",
        ElementKind::Container => "container",
        ElementKind::Component => "component",
        ElementKind::Environment => "environment",
        ElementKind::DeploymentNode => "deployment-node",
        ElementKind::ContainerInstance => "container-instance",
    }
}

/// One element in snapshot form — shared by the full snapshot and the diff
/// payload (removed-element ghosts).
pub fn snap_element(e: &crate::model::Element) -> SnapElement {
    SnapElement {
        id: e.id.clone(),
        kind: kind_str(e.kind),
        parent: e.id.rsplit_once('.').map(|(p, _)| p.to_string()),
        name: e.name.clone(),
        tech: e.tech.clone(),
        description: e.description.clone(),
        external: e.external || e.kind == ElementKind::External,
        group: e.group.clone(),
        replicas: e.replicas,
        source: e.source.as_ref().map(|m| SnapSource {
            language: m.language.clone(),
            root: m.root.clone(),
            include: m.include.clone(),
            exclude: m.exclude.clone(),
            mode: m.mode.clone(),
            extractor: m.extractor.clone(),
        }),
    }
}

/// Build the snapshot. The source is needed to read doc bodies — the model
/// keeps only frontmatter (ADR-0010: bodies belong to the user's editor).
pub fn snapshot(vfs: &dyn Vfs, ws: &Workspace, diags: &[Diagnostic]) -> Snapshot {
    let elements = ws.elements.values().map(snap_element).collect();

    let relations = ws
        .relations
        .iter()
        .map(|r| {
            let scope = r.scope.as_deref();
            SnapRelation {
                from: ws.resolve(&r.from, scope).unwrap_or_else(|| r.from.clone()),
                to: ws.resolve(&r.to, scope).unwrap_or_else(|| r.to.clone()),
                label: r.label.clone(),
                protocol: r.protocol.clone(),
                direction: match r.direction {
                    Direction::Forward => "forward",
                    Direction::Both => "both",
                    Direction::None => "none",
                },
            }
        })
        .collect();

    let views = ws
        .views
        .iter()
        .map(|v| SnapView {
            id: v.id.clone(),
            name: v.name.clone(),
            scope: v.scope.clone(),
            level: v.level.clone(),
            layout: v.layout.clone(),
            include_context: v.include_context,
            show_groups: v.show_groups,
            descriptions: v.descriptions.iter().cloned().collect(),
            nested: v.nested,
        })
        .collect();

    let docs = ws
        .docs
        .iter()
        .map(|d| {
            let body = vfs
                .read(&d.file)
                .map(|t| strip_frontmatter(&t))
                .unwrap_or_default();
            SnapDoc {
                title: body
                    .lines()
                    .find_map(|l| l.strip_prefix("# ").map(str::to_string))
                    .unwrap_or_else(|| d.id.clone()),
                id: d.id.clone(),
                doc_type: d.doc_type.clone(),
                status: d.status.clone(),
                elements: d.elements.clone(),
                file: d.file.clone(),
                body,
            }
        })
        .collect();

    let derived = ws
        .derived
        .iter()
        .map(|g| SnapDerived {
            component: g.component.clone(),
            language: g.language.clone(),
            stale: g.stale,
            elements: g
                .elements
                .iter()
                .map(|e| SnapDerivedElement {
                    id: e.id.clone(),
                    kind: e.kind.clone(),
                    name: e.name.clone(),
                    parent: e.parent.clone(),
                    path: e.path.clone(),
                    line: e.line,
                })
                .collect(),
            edges: g
                .edges
                .iter()
                .map(|e| SnapDerivedEdge { from: e.from.clone(), to: e.to.clone(), kind: e.kind.clone() })
                .collect(),
        })
        .collect();

    let drift = crate::drift::detect(ws)
        .into_iter()
        .map(|d| SnapDrift {
            from: d.from,
            to: d.to,
            kind: match d.kind {
                crate::drift::DriftKind::Undeclared => "undeclared",
                crate::drift::DriftKind::Unbacked => "unbacked",
            },
            via: d.via,
        })
        .collect();

    Snapshot {
        name: ws.name.clone(),
        elements,
        relations,
        views,
        docs,
        derived,
        drift,
        diagnostics: diags
            .iter()
            .map(|d| SnapDiagnostic {
                severity: d.severity.to_string(),
                file: d.file.clone(),
                line: d.line,
                message: d.message.clone(),
            })
            .collect(),
    }
}

fn strip_frontmatter(text: &str) -> String {
    let Some(rest) = text.strip_prefix("---") else {
        return text.to_string();
    };
    let Some(rest) = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n')) else {
        return text.to_string();
    };
    for (idx, _) in rest.match_indices("---") {
        let at_line_start = idx == 0 || rest.as_bytes().get(idx - 1) == Some(&b'\n');
        if at_line_start {
            let after = &rest[idx + 3..];
            return after.trim_start_matches(['\r', '\n']).to_string();
        }
    }
    text.to_string()
}
