//! blastradius — CLI over blastradius-core (ADR-0005: the core is a library;
//! this binary and CI attach to it without a WebView).

use blastradius_core::diagnostics::{has_errors, Severity};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate") => validate(args.get(1).map(String::as_str).unwrap_or(".")),
        Some("diff") => match (args.get(1), args.get(2)) {
            (Some(a), Some(b)) => diff(a, b),
            _ => usage(),
        },
        Some("snapshot") => snapshot(args.get(1).map(String::as_str).unwrap_or(".")),
        Some("gitdiff") => match args.get(1) {
            Some(dir) => gitdiff(dir, args.get(2).map(String::as_str), args.get(3).map(String::as_str)),
            None => usage(),
        },
        Some("export") => export(&args[1..]),
        Some("init") => init(&args[1..]),
        Some("mcp") => match blastradius_cli::mcp::serve(args.get(1).map(String::as_str)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("import") => match (args.get(1), args.get(2)) {
            (Some(dsl), Some(out)) => import(dsl, out),
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  blastradius init [dir] [--name <name>]\n  blastradius validate [workspace-dir]\n  blastradius diff <base-dir> <current-dir>\n  blastradius gitdiff <dir> [base-ref] [cur-ref]\n  blastradius snapshot [workspace-dir]\n  blastradius export <dir> -o <file.html> [--with-doc-bodies]\n  blastradius import <workspace.dsl> <out-dir>\n  blastradius mcp [workspace-dir]"
    );
    ExitCode::from(2)
}

/// Emit the renderer snapshot as JSON on stdout — the same shape the Tauri
/// shell serves over IPC, so frontend work can run against real data headless.
fn snapshot(dir: &str) -> ExitCode {
    let root = Path::new(dir);
    let (ws, diags) = blastradius_core::load_workspace(root);
    let vfs = blastradius_core::vfs::DiskVfs::new(root);
    let snap = blastradius_core::snapshot::snapshot(&vfs, &ws, &diags);
    println!("{}", serde_json::to_string_pretty(&snap).expect("snapshot serializes"));
    if has_errors(&diags) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn validate(dir: &str) -> ExitCode {
    let (ws, diags) = blastradius_core::load_workspace(Path::new(dir));

    let views = ws.views.len();
    let (mut infos, mut warns, mut errs) = (0, 0, 0);
    for d in &diags {
        match d.severity {
            Severity::Info => infos += 1,
            Severity::Warning => warns += 1,
            Severity::Error => errs += 1,
        }
        println!("{d}");
    }
    println!(
        "{}: {} elements, {} relations, {} views, {} docs — {} error(s), {} warning(s), {} info",
        ws.name,
        ws.elements.len(),
        ws.relations.len(),
        views,
        ws.docs.len(),
        errs,
        warns,
        infos
    );
    if has_errors(&diags) {
        println!("RESULT: FAIL");
        ExitCode::FAILURE
    } else {
        println!("RESULT: PASS");
        ExitCode::SUCCESS
    }
}

fn diff(base_dir: &str, current_dir: &str) -> ExitCode {
    let (base, base_diags) = blastradius_core::load_workspace(Path::new(base_dir));
    let (cur, cur_diags) = blastradius_core::load_workspace(Path::new(current_dir));
    if has_errors(&base_diags) || has_errors(&cur_diags) {
        for d in base_diags.iter().chain(&cur_diags) {
            if d.severity == Severity::Error {
                eprintln!("{d}");
            }
        }
        eprintln!("cannot diff: one side is invalid");
        return ExitCode::from(2);
    }
    let d = blastradius_core::diff::diff(&base, &cur);
    for (id, change) in &d.elements {
        println!("{} element {id}", tag(*change));
    }
    for ((from, to, label), change) in &d.relations {
        let label = if label.is_empty() { String::new() } else { format!(" ({label})") };
        println!("{} relation {from} -> {to}{label}", tag(*change));
    }
    if d.is_empty() {
        println!("no semantic changes");
    }
    ExitCode::SUCCESS
}

fn tag(c: blastradius_core::diff::Change) -> &'static str {
    use blastradius_core::diff::Change::*;
    match c {
        Added => "+",
        Removed => "-",
        Changed => "~",
    }
}

/// Semantic diff from git history (spec/git-and-diff.md): base defaults to
/// the merge-base with the default branch; current defaults to the working
/// tree.
fn gitdiff(dir: &str, base_ref: Option<&str>, cur_ref: Option<&str>) -> ExitCode {
    use blastradius_core::git::GitContext;
    let root = Path::new(dir);
    let Some(ctx) = GitContext::discover(root) else {
        eprintln!("{dir}: not inside a git repository");
        return ExitCode::from(2);
    };
    let base_label = base_ref
        .map(str::to_string)
        .or_else(|| ctx.default_base())
        .unwrap_or_else(|| "HEAD".to_string());

    let (base_ws, base_diags) = match ctx.load_at(&base_label) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot load base {base_label}: {e}");
            return ExitCode::from(2);
        }
    };
    let (cur_ws, cur_diags) = match cur_ref {
        Some(r) => match ctx.load_at(r) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("cannot load {r}: {e}");
                return ExitCode::from(2);
            }
        },
        None => blastradius_core::load_workspace(root),
    };
    if has_errors(&base_diags) || has_errors(&cur_diags) {
        eprintln!("cannot diff: one side is invalid");
        return ExitCode::from(2);
    }

    let payload = blastradius_core::diff::diff_payload(&base_label, &base_ws, &cur_ws);
    for e in &payload.elements {
        println!("{} element {}", tag_str(e.change), e.id);
    }
    for r in &payload.relations {
        let label = r.label.as_deref().map(|l| format!(" ({l})")).unwrap_or_default();
        println!("{} relation {} -> {}{label}", tag_str(r.change), r.from, r.to);
    }
    for l in &payload.layout {
        println!("~ layout {} [{}]", l.view, l.pins.join(", "));
    }
    if payload.elements.is_empty() && payload.relations.is_empty() && payload.layout.is_empty() {
        println!("no semantic changes vs {base_label}");
    }
    ExitCode::SUCCESS
}

fn tag_str(c: &str) -> &'static str {
    match c {
        "added" => "+",
        "removed" => "-",
        _ => "~",
    }
}

/// Self-contained HTML export (ADR-0009) — the CI story: publish the model as
/// a build artifact on every merge. Headless: layout runs in the viewer.
fn export(args: &[String]) -> ExitCode {
    let mut dir = None;
    let mut out = None;
    let mut bodies = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--with-doc-bodies" => bodies = true,
            "-o" => out = it.next().cloned(),
            other => dir = Some(other.to_string()),
        }
    }
    let (Some(dir), Some(out)) = (dir, out) else {
        return usage();
    };
    let root = Path::new(&dir);
    let (ws, diags) = blastradius_core::load_workspace(root);
    if has_errors(&diags) {
        for d in &diags {
            eprintln!("{d}");
        }
        eprintln!("cannot export: workspace is invalid");
        return ExitCode::FAILURE;
    }
    let vfs = blastradius_core::vfs::DiskVfs::new(root);
    let options = blastradius_core::export::ExportOptions { include_doc_bodies: bodies };
    match blastradius_core::export::export_html(&vfs, &ws, &diags, &options) {
        Ok(html) => {
            if let Some(parent) = Path::new(&out).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&out, &html) {
                Ok(()) => {
                    println!("{out}: {} KB, self-contained, zero network requests", html.len() / 1024);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cannot write {out}: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("export failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Scaffold a starter workspace (Phase 5 onboarding). Refuses to touch a
/// folder that already has a manifest; never overwrites any existing file.
fn init(args: &[String]) -> ExitCode {
    let mut dir = None;
    let mut name = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => name = it.next().cloned(),
            other => dir = Some(other.to_string()),
        }
    }
    let dir = dir.unwrap_or_else(|| ".".to_string());
    let root = Path::new(&dir);
    if root.join("workspace.yaml").is_file() {
        eprintln!("{dir}: already a Blastradius workspace (workspace.yaml exists)");
        return ExitCode::from(2);
    }
    let name = name.unwrap_or_else(|| {
        root.canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "My System".to_string())
    });
    for (rel, text) in blastradius_core::scaffold::starter_workspace(&name) {
        let path = root.join(&rel);
        if path.exists() {
            eprintln!("{rel}: exists — refusing to overwrite");
            return ExitCode::from(2);
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("cannot write {rel}: {e}");
            return ExitCode::FAILURE;
        }
        println!("  created {rel}");
    }
    let (ws, diags) = blastradius_core::load_workspace(root);
    if has_errors(&diags) {
        for d in &diags {
            eprintln!("{d}");
        }
        eprintln!("scaffold does not validate — this is a bug, please report it");
        return ExitCode::FAILURE;
    }
    println!(
        "{}: {} elements, {} views — next:
  blastradius-app {dir}    # open it in the app
  blastradius validate {dir}",
        ws.name,
        ws.elements.len(),
        ws.views.len()
    );
    ExitCode::SUCCESS
}

/// One-way Structurizr DSL import with fidelity report (ADR-0002).
fn import(dsl_path: &str, out_dir: &str) -> ExitCode {
    let src = match std::fs::read_to_string(dsl_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {dsl_path}: {e}");
            return ExitCode::from(2);
        }
    };
    let imported = match blastradius_core::import::import_dsl(&src) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("import failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = Path::new(out_dir);
    for (rel, text) in &imported.files {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    // the imported workspace must validate — that is what "clean import" means
    let (ws, diags) = blastradius_core::load_workspace(out);
    for d in &diags {
        println!("{d}");
    }
    println!(
        "imported {:?}: {} elements, {} files, {} constructs not mapped (see import-report.md)",
        imported.workspace_name,
        ws.elements.len(),
        imported.files.len(),
        imported.fidelity.skipped.len(),
    );
    if has_errors(&diags) {
        println!("RESULT: IMPORTED WITH ERRORS");
        ExitCode::FAILURE
    } else {
        println!("RESULT: CLEAN");
        ExitCode::SUCCESS
    }
}
