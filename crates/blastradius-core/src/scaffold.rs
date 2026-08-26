//! `blastradius init` (Phase 5 onboarding): a starter workspace that teaches
//! the format by example. Every file it emits validates cleanly and carries
//! the comments a newcomer needs to keep going without the docs open.

use std::path::Path;

/// What `scaffold_into` did, so a caller can say so precisely.
pub struct Scaffolded {
    pub created: Vec<String>,
    /// Files that were already there. Left exactly as they were.
    pub skipped: Vec<String>,
}

/// Write the starter workspace into `root`, **leaving any existing file
/// untouched**.
///
/// An existing file is not a conflict, it is the user's file. Both surfaces
/// used to treat one as fatal — and the starter set includes `README.md`,
/// which essentially every repository already has, so "start a model here"
/// failed on any real repo: the app left its dialog open having written
/// nothing and skipped the agent setup entirely, and `blastradius init .`
/// wrote four files, exited 2, and skipped it too (reported 2026-08-26).
///
/// The workspace is valid without whichever files were skipped — the README
/// is a pointer, not part of the model — so partial scaffolding is a normal
/// outcome, not a degraded one.
pub fn scaffold_into(root: &Path, name: &str) -> Result<Scaffolded, String> {
    let mut out = Scaffolded { created: Vec::new(), skipped: Vec::new() };
    for (rel, text) in starter_workspace(name) {
        let path = root.join(&rel);
        if path.exists() {
            out.skipped.push(rel);
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("cannot write {rel}: {e}"))?;
        out.created.push(rel);
    }
    Ok(out)
}

/// Folder names a project might already keep its documentation in, most
/// conventional first.
const DOC_DIRS: [&str; 2] = ["docs", "doc"];

/// Where a new workspace should go inside a project folder, relative to it.
///
/// A repository root is for source; the model is documentation and belongs
/// with the documentation — scattering `blastradius.yaml`, `model/` and
/// `views/` through someone's root is untidy, and it is not what this
/// repository does with its own model either (`docs/`).
///
/// If the project already keeps docs somewhere — `docs/` or `doc/` — that is
/// the recommendation, so we never create a near-duplicate of a folder that
/// is already there. Otherwise `docs`.
///
/// Only ever a *recommendation*: both surfaces ask, and `.` is always a valid
/// answer.
pub fn suggested_location(root: &Path) -> String {
    DOC_DIRS
        .iter()
        .find(|d| root.join(d).is_dir())
        .unwrap_or(&DOC_DIRS[0])
        .to_string()
}

/// Validate a location a user typed. Relative, inside the project, no
/// climbing out — a workspace path is not a place to accept `..` or `C:\`.
pub fn check_location(location: &str) -> Result<(), String> {
    let t = location.trim();
    if t.is_empty() {
        return Err("no location given".into());
    }
    if t == "." {
        return Ok(());
    }
    let p = Path::new(t);
    if p.is_absolute() || t.starts_with('/') || t.starts_with('\\') {
        return Err(format!("{t:?}: give a folder inside the project, not an absolute path"));
    }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(format!("{t:?}: cannot contain `..`"));
    }
    Ok(())
}

/// The repo (or folder) name, as the starter model's system name.
pub fn name_for(root: &Path) -> String {
    root.canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .or_else(|| root.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "My System".to_string())
}

/// Slug for ids and file names (ADR-0003 charset).
pub fn slugify(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "system".to_string()
    } else {
        s
    }
}

/// The starter workspace as (relative path, content) pairs. `name` is the
/// human name of the system being modelled (typically the repo name).
pub fn starter_workspace(name: &str) -> Vec<(String, String)> {
    let slug = slugify(name);
    let mut files = Vec::new();

    files.push((
        "blastradius.yaml".to_string(),
        format!(
            "# {name} — Blastradius workspace manifest.\n\
             # The model is plain YAML in this folder: the app, the CLI, and CI all read\n\
             # the same files, and git versions them like any other source.\n\
             workspace:\n\
             \x20 name: {name}\n\
             \x20 version: 1\n\
             model:\n\
             \x20 include: [model/*.yaml]\n\
             views:\n\
             \x20 include: [views/*.yaml]\n\
             docs:\n\
             \x20 include: [\"*.md\"]\n"
        ),
    ));

    files.push((
        "model/context.yaml".to_string(),
        format!(
            "# The context (C4 L1): who uses the system and what it talks to.\n\
             # YAML keys are immutable ids (ADR-0003) — rename via `name:` only.\n\
             people:\n\
             \x20 user:\n\
             \x20   name: User\n\
             \x20   description: Replace me — who uses {name}, and why.\n\
             \n\
             external:\n\
             \x20 third-party:\n\
             \x20   name: Third-Party Service\n\
             \x20   description: A system outside your control that {name} depends on.\n\
             \n\
             relations:\n\
             \x20 - from: user\n\
             \x20   to: {slug}\n\
             \x20   label: uses\n"
        ),
    ));

    files.push((
        format!("model/{slug}.yaml"),
        format!(
            "# One file per software system (spec §3). Containers are the things that\n\
             # run (C4 L2); nest `components:` under a container for L3.\n\
             system: {slug}\n\
             name: {name}\n\
             description: Replace me — what {name} does, for whom.\n\
             \n\
             containers:\n\
             \x20 app:\n\
             \x20   name: Application\n\
             \x20   description: The deployable that does the work.\n\
             \x20 db:\n\
             \x20   name: Database\n\
             \x20   tech: PostgreSQL\n\
             \n\
             relations:\n\
             \x20 # Endpoints are relative to this system where possible; absolute ids\n\
             \x20 # (like the external above) also work.\n\
             \x20 - from: app\n\
             \x20   to: db\n\
             \x20   label: reads and writes\n\
             \x20   protocol: SQL\n\
             \x20 - from: app\n\
             \x20   to: third-party\n\
             \x20   label: calls\n\
             \x20   protocol: HTTPS\n"
        ),
    ));

    files.push((
        "views/containers.yaml".to_string(),
        format!(
            "# Pin what you care about; everything absent from `layout:` is auto-placed\n\
             # deterministically (ADR-0006). Dragging nodes in the app writes pins here.\n\
             view: containers\n\
             name: Containers\n\
             scope: {slug}\n\
             level: L2\n\
             layout:\n\
             \x20 app: [4, 2]\n\
             include-context: true\n"
        ),
    ));

    files.push((
        "README.md".to_string(),
        format!(
            "---\n\
             doc: readme\n\
             type: note\n\
             elements: [{slug}]\n\
             ---\n\
             \n\
             # {name} architecture\n\
             \n\
             This folder is a [Blastradius](https://github.com/kommundsen/blastradius)\n\
             workspace. Edit the YAML here or drag things in the app — both stay in\n\
             sync, and git sees ordinary text diffs either way.\n\
             \n\
             - `blastradius validate .` checks the model (wire it into CI)\n\
             - `blastradius export . -o architecture.html` produces a self-contained\n\
             \x20 shareable page\n\
             - Markdown files with a `doc:` frontmatter block, like this one, are part\n\
             \x20 of the model: `elements:` links them to model elements, and dangling\n\
             \x20 links are validation errors.\n"
        ),
    ));

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_workspace_validates_cleanly() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "blastradius-scaffold-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, text) in starter_workspace("Acme Payments") {
            let path = dir.join(&rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let (ws, diags) = crate::load_workspace(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == crate::diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "scaffold must validate: {errors:?}");
        assert_eq!(ws.name, "Acme Payments");
        assert!(ws.elements.contains_key("acme-payments"));
        assert!(ws.elements.contains_key("acme-payments.app"));
        assert!(!ws.views.is_empty(), "starter includes a pinned view");
        assert!(!ws.docs.is_empty(), "starter README registers as a doc");
    }

    #[test]
    fn slugify_handles_awkward_names() {
        assert_eq!(slugify("Acme Payments"), "acme-payments");
        assert_eq!(slugify("--Weird__NAME 2!"), "weird-name-2");
        assert_eq!(slugify("§§§"), "system");
    }
}

/// Deterministic benchmark workspace for the CI performance budgets
/// (spec/sync-engine.md): `systems` systems × (1 + 4 containers + 20
/// components) elements, plus context, relations, and pinned views. 20
/// systems ≈ a 510-element workspace.
pub fn benchmark_workspace(systems: usize) -> Vec<(String, String)> {
    let mut files = Vec::new();
    files.push((
        "blastradius.yaml".to_string(),
        "workspace:\n  name: Benchmark\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n".to_string(),
    ));
    let mut ctx = String::from("people:\n");
    for i in 0..5 {
        ctx.push_str(&format!("  user-{i}:\n    name: User {i}\n"));
    }
    ctx.push_str("external:\n");
    for i in 0..5 {
        ctx.push_str(&format!("  vendor-{i}:\n    name: Vendor {i}\n"));
    }
    ctx.push_str("relations:\n");
    for i in 0..5 {
        ctx.push_str(&format!("  - from: user-{i}\n    to: sys-{}\n    label: uses\n", i % systems));
    }
    files.push(("model/context.yaml".to_string(), ctx));

    for s in 0..systems {
        let mut f = format!("system: sys-{s}\nname: System {s}\ncontainers:\n");
        for c in 0..4 {
            f.push_str(&format!("  svc-{c}:\n    name: Service {c}\n    tech: Rust\n    components:\n"));
            for k in 0..5 {
                f.push_str(&format!("      mod-{k}:\n        name: Module {k}\n"));
            }
        }
        f.push_str("relations:\n");
        for c in 0..3 {
            f.push_str(&format!("  - from: svc-{c}\n    to: svc-{}\n    label: calls\n", c + 1));
        }
        f.push_str(&format!("  - from: svc-0\n    to: sys-{}.svc-0\n    label: federates\n", (s + 1) % systems));
        f.push_str(&format!("  - from: svc-3\n    to: vendor-{}\n    label: buys from\n", s % 5));
        files.push((format!("model/sys-{s}.yaml"), f));

        files.push((
            format!("views/sys-{s}-l2.yaml"),
            format!("view: sys-{s}-l2\nscope: sys-{s}\nlevel: L2\nlayout:\n  svc-0: [2, 2]\n  svc-1: [8, 2]\n"),
        ));
    }
    files.push((
        "views/sys-0-l3.yaml".to_string(),
        "view: sys-0-l3\nscope: sys-0.svc-0\nlevel: L3\ninclude-context: false\n".to_string(),
    ));
    files
}
