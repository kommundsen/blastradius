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
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  blastradius validate [workspace-dir]\n  blastradius diff <base-dir> <current-dir>\n  blastradius snapshot [workspace-dir]"
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
