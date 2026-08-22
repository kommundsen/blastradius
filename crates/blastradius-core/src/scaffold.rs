//! `blastradius init` (Phase 5 onboarding): a starter workspace that teaches
//! the format by example. Every file it emits validates cleanly and carries
//! the comments a newcomer needs to keep going without the docs open.

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
        "workspace.yaml".to_string(),
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
