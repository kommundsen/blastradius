//! Blastradius model core — workspace loading, schema validation, semantic diff.
//!
//! Library-first (ADR-0005): the CLI, the future Tauri shell, and CI all attach
//! here. Nothing in this crate touches a terminal, a WebView, or git.
//!
//! The normative schema reference is `docs/spec/model-format.md`; the `docs/`
//! folder of this repository is the conformance workspace (the dogfood gate).

pub mod diagnostics;
pub mod diff;
pub mod docs;
pub mod manifest;
pub mod model;
pub mod parse;
pub mod validate;
pub mod views;
mod yaml;

use std::path::Path;

pub use diagnostics::{Diagnostic, Severity};
pub use diff::{Change, ModelDiff};
pub use model::Workspace;

/// Highest schema version this build understands (spec §1).
pub const SCHEMA_VERSION: u64 = 1;

/// Load a workspace folder (the directory containing `workspace.yaml`).
///
/// Always returns the best-effort workspace plus every diagnostic gathered on
/// the way; the workspace is usable when no diagnostic is `Severity::Error`.
pub fn load_workspace(root: &Path) -> (Workspace, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let mut ws = Workspace::default();

    let manifest = match manifest::load(root, &mut diags) {
        Some(m) => m,
        None => return (ws, diags), // unreadable/invalid manifest: diagnostics explain
    };
    ws.name = manifest.name.clone();

    for file in &manifest.model_files {
        parse::parse_model_file(root, file, &mut ws, &mut diags);
    }
    for file in &manifest.view_files {
        views::parse_view_file(root, file, &mut ws, &mut diags);
    }
    for file in &manifest.doc_files {
        docs::parse_doc_file(root, file, &mut ws, &mut diags);
    }

    validate::cross_validate(&ws, &mut diags);
    (ws, diags)
}
