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
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  blastradius validate [workspace-dir]\n  blastradius diff <base-dir> <current-dir>"
    );
    ExitCode::from(2)
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
