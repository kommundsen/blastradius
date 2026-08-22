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
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Snapshot {
    pub name: String,
    pub elements: Vec<SnapElement>,
    pub relations: Vec<SnapRelation>,
    pub views: Vec<SnapView>,
    pub docs: Vec<SnapDoc>,
    pub diagnostics: Vec<SnapDiagnostic>,
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
    }
}

/// Build the snapshot. `root` is needed to read doc bodies — the model keeps
/// only frontmatter (ADR-0010: bodies belong to the user's editor).
pub fn snapshot(root: &Path, ws: &Workspace, diags: &[Diagnostic]) -> Snapshot {
    let elements = ws
        .elements
        .values()
        .map(|e| SnapElement {
            id: e.id.clone(),
            kind: kind_str(e.kind),
            parent: e.id.rsplit_once('.').map(|(p, _)| p.to_string()),
            name: e.name.clone(),
            tech: e.tech.clone(),
            description: e.description.clone(),
            external: e.external || e.kind == ElementKind::External,
        })
        .collect();

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
        })
        .collect();

    let docs = ws
        .docs
        .iter()
        .map(|d| {
            let body = std::fs::read_to_string(root.join(&d.file))
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

    Snapshot {
        name: ws.name.clone(),
        elements,
        relations,
        views,
        docs,
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
