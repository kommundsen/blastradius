//! blastradius — CLI over blastradius-core (ADR-0005: the core is a library;
//! this binary and CI attach to it without a WebView).

use blastradius_core::diagnostics::{has_errors, Severity};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate") => {
            // --strict-drift is how CI opts in: drift stays a warning by
            // default so an existing repo is not red on day one (ADR-0019).
            let strict = args.iter().any(|a| a == "--strict-drift");
            let dir = args.iter().skip(1).find(|a| !a.starts_with("--"));
            resolving(dir, move |d| validate(d, strict))
        }
        Some("diff") => match (args.get(1), args.get(2)) {
            (Some(a), Some(b)) => match (resolve(a), resolve(b)) {
                (Ok(a), Ok(b)) => diff(&a, &b),
                (Err(c), _) | (_, Err(c)) => c,
            },
            _ => usage(),
        },
        Some("snapshot") => resolving(args.get(1), snapshot),
        Some("gitdiff") => match args.get(1) {
            Some(dir) => match resolve(dir) {
                Ok(dir) => gitdiff(&dir, args.get(2).map(String::as_str), args.get(3).map(String::as_str)),
                Err(c) => c,
            },
            None => usage(),
        },
        // The schema, from the binary that enforces it. Without this the only
        // authoritative copy lives in the Blastradius repository, which is
        // nowhere near the user editing their own workspace.
        Some("format") => {
            println!("{}", blastradius_cli::format_ref::full_reference());
            ExitCode::SUCCESS
        }
        Some("export") => export(&args[1..]),
        Some("introspect") => introspect(&args[1..]),
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

/// Workspace-taking commands accept a repo root too: discovery finds the
/// workspace below it (ADR-0014); ambiguity is an error listing the hits.
fn resolve(dir: &str) -> Result<String, ExitCode> {
    match blastradius_cli::mcp::resolve_root(Some(dir)) {
        Ok(p) => Ok(p.display().to_string()),
        Err(e) => {
            eprintln!("{e}");
            Err(ExitCode::from(2))
        }
    }
}

fn resolving(dir: Option<&String>, run: impl FnOnce(&str) -> ExitCode) -> ExitCode {
    match resolve(dir.map(String::as_str).unwrap_or(".")) {
        Ok(d) => run(&d),
        Err(c) => c,
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  blastradius init [dir] [--name <name>]\n  blastradius format\n  blastradius validate [workspace-dir] [--strict-drift]\n  blastradius diff <base-dir> <current-dir>\n  blastradius gitdiff <dir> [base-ref] [cur-ref]\n  blastradius snapshot [workspace-dir]\n  blastradius export <dir> -o <file.html> [--with-doc-bodies]\n  blastradius introspect [dir] [component-id] [--check]\n  blastradius import <workspace.dsl> <out-dir>\n  blastradius mcp [workspace-dir]"
    );
    ExitCode::from(2)
}

/// Extract L4 facts for opted-in components (spec/l4-introspection.md).
/// `--check` regenerates without writing and fails on drift — the CI
/// staleness gate, same pattern as the snapshot gate.
fn introspect(args: &[String]) -> ExitCode {
    use blastradius_core::introspect as intro;
    use blastradius_core::model::ElementKind;

    let check = args.iter().any(|a| a == "--check");
    let pos: Vec<&str> = args.iter().filter(|a| !a.starts_with("--")).map(String::as_str).collect();
    let dir = match resolve(pos.first().copied().unwrap_or(".")) {
        Ok(d) => d,
        Err(c) => return c,
    };
    let only = pos.get(1).copied();

    let ws_dir = Path::new(&dir);
    let (ws, diags) = blastradius_core::load_workspace(ws_dir);
    if has_errors(&diags) {
        for d in &diags {
            if d.severity == Severity::Error {
                eprintln!("{d}");
            }
        }
        eprintln!("workspace has errors — fix them before introspecting");
        return ExitCode::FAILURE;
    }
    let Some(repo_root) = intro::find_repo_root(ws_dir) else {
        eprintln!("no repository root found above {dir} — `source:` roots are repo-root-relative (ADR-0014)");
        return ExitCode::FAILURE;
    };

    let targets: Vec<_> = ws
        .elements
        .values()
        .filter(|e| e.kind == ElementKind::Component && e.source.is_some())
        .filter(|e| only.is_none_or(|id| e.id == id))
        .collect();
    if targets.is_empty() {
        match only {
            Some(id) => {
                eprintln!("{id:?} is not a component with a `source:` mapping");
                return ExitCode::FAILURE;
            }
            None => {
                println!("no components opt into introspection — nothing to do");
                return ExitCode::SUCCESS;
            }
        }
    }

    let mut drift = false;
    let mut failed = false;
    for comp in targets {
        let mapping = comp.source.as_ref().expect("filtered");
        match intro::extract(&repo_root, &comp.id, mapping) {
            Ok((facts, warnings)) => {
                for w in warnings {
                    eprintln!("warning: {}: {w}", comp.id);
                }
                let bytes = intro::facts_bytes(&facts);
                let path = ws_dir.join("model").join("derived").join(format!("{}.l4.json", comp.id));
                let existing = std::fs::read_to_string(&path).ok().map(|t| t.replace("\r\n", "\n"));
                if check {
                    if existing.as_deref() == Some(bytes.as_str()) {
                        println!("{}: up to date", comp.id);
                    } else if fell_back_from_semantic(existing.as_deref(), &facts.extractor) {
                        // The committed facts were extracted with a semantic
                        // pass this machine cannot run. Comparing them would
                        // report drift that no edit caused, so say what is
                        // actually true (spec/l4-introspection.md).
                        println!(
                            "{}: NOT VERIFIED — committed facts are semantic; this machine fell back to syntax",
                            comp.id
                        );
                    } else {
                        println!("{}: STALE — run `blastradius introspect` and commit the result", comp.id);
                        drift = true;
                    }
                } else if existing.as_deref() == Some(bytes.as_str()) {
                    println!("{}: unchanged", comp.id);
                } else {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            eprintln!("{}: {e}", parent.display());
                            failed = true;
                            continue;
                        }
                    }
                    match std::fs::write(&path, &bytes) {
                        Ok(()) => println!(
                            "{}: wrote {} ({} elements, {} edges)",
                            comp.id,
                            path.display(),
                            facts.elements.len(),
                            facts.edges.len()
                        ),
                        Err(e) => {
                            eprintln!("{}: {e}", path.display());
                            failed = true;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: {e}", comp.id);
                failed = true;
            }
        }
    }
    if failed || drift {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
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

fn validate(dir: &str, strict_drift: bool) -> ExitCode {
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
    let drift = blastradius_core::drift::detect(&ws);
    if strict_drift && !drift.is_empty() {
        println!("RESULT: FAIL — {} drift finding(s); the model and the code disagree", drift.len());
        return ExitCode::FAILURE;
    }
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
    let dir = match resolve(&dir) {
        Ok(d) => d,
        Err(c) => return c,
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

/// Scaffold a starter workspace (Phase 5 onboarding), then offer the
/// repo-level extras: `git init`, MCP registration, and agent skills.
/// Idempotent: an existing workspace skips the scaffold but still gets the
/// extras; no existing file is ever overwritten.
/// True when committed facts record a semantic extraction but this run could
/// only manage syntax — a machine capability gap, not stale facts. The
/// extractor string carries the effective mode for exactly this reason.
fn fell_back_from_semantic(existing: Option<&str>, extractor: &str) -> bool {
    extractor.contains("(syntax-fallback)")
        && existing.is_some_and(|t| t.contains("(semantic)"))
}

fn init(args: &[String]) -> ExitCode {
    use std::io::IsTerminal;
    let mut dir = None;
    let mut name = None;
    let mut git_flag: Option<bool> = None;
    let mut agents_flag: Option<String> = None;
    let mut skills_flag: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => name = it.next().cloned(),
            "--git" => git_flag = Some(true),
            "--no-git" => git_flag = Some(false),
            "--agents" => agents_flag = Some(it.next().cloned().unwrap_or_default()),
            "--skills" => skills_flag = Some(it.next().cloned().unwrap_or_default()),
            other => dir = Some(other.to_string()),
        }
    }
    let dir = dir.unwrap_or_else(|| ".".to_string());
    let root = Path::new(&dir);

    let fresh = !root.join("blastradius.yaml").is_file() && !root.join("workspace.yaml").is_file();
    if fresh {
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
        println!("{}: {} elements, {} views", ws.name, ws.elements.len(), ws.views.len());
    } else {
        println!("{dir}: already a workspace — scaffold skipped");
    }

    // ---- repo-level extras --------------------------------------------------
    let interactive = git_flag.is_none()
        && agents_flag.is_none()
        && skills_flag.is_none()
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal();

    let in_repo = blastradius_cli::onboard::git_root(root).is_some();
    let git_init = if in_repo {
        false
    } else {
        match git_flag {
            Some(v) => v,
            None if interactive => {
                ask_yes("Not a git repository — run `git init`? [Y/n] ", true)
            }
            None => false,
        }
    };
    let choices = "all / none / any of claude,copilot,cursor,codex";
    let mcp = match agents_flag.or_else(|| {
        interactive.then(|| ask_line(&format!(
            "Configure MCP so coding agents can query the model? ({choices}) [none] "
        )))
    }) {
        Some(spec) => match parse_agents(&spec) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        },
        None => Vec::new(),
    };
    let skills = match skills_flag.or_else(|| {
        interactive.then(|| ask_line(&format!(
            "Add agent skills/instructions for the model? ({choices}) [none] "
        )))
    }) {
        Some(spec) => match parse_agents(&spec) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        },
        None => Vec::new(),
    };

    // command: None — the CLI writes the bare name, which resolves for anyone
    // who could type `blastradius init` in the first place.
    let opts =
        blastradius_cli::onboard::SetupOptions { git_init, mcp, skills, command: None };
    if opts.git_init || !opts.mcp.is_empty() || !opts.skills.is_empty() {
        for line in blastradius_cli::onboard::setup(root, &opts) {
            println!("  {line}");
        }
    }
    println!("next:\n  blastradius-app {dir}    # open it in the app\n  blastradius validate {dir}");
    ExitCode::SUCCESS
}

fn parse_agents(spec: &str) -> Result<Vec<String>, String> {
    let spec = spec.trim().to_lowercase();
    if spec.is_empty() || spec == "none" || spec == "n" {
        return Ok(Vec::new());
    }
    if spec == "all" || spec == "a" {
        return Ok(blastradius_cli::onboard::AGENTS.iter().map(|s| s.to_string()).collect());
    }
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if !blastradius_cli::onboard::AGENTS.contains(&part) {
            return Err(format!(
                "unknown agent {part:?} — expected all, none, or any of {}",
                blastradius_cli::onboard::AGENTS.join(", ")
            ));
        }
        if !out.contains(&part.to_string()) {
            out.push(part.to_string());
        }
    }
    Ok(out)
}

fn ask_yes(prompt: &str, default: bool) -> bool {
    let answer = ask_line(prompt);
    let answer = answer.trim().to_lowercase();
    match answer.as_str() {
        "" => default,
        "y" | "yes" => true,
        _ => false,
    }
}

fn ask_line(prompt: &str) -> String {
    use std::io::Write;
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line
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
