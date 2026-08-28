//! Blastradius desktop shell. Phase 3: the sync engine arrives — canvas and
//! YAML panel propose transactions; files stay the single source of truth
//! (ADR-0008). Git remains read-only (ADR-0007).

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use blastradius_core::git::GitContext;
use blastradius_core::sync::{Operation, SyncEngine};
use notify::Watcher;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};

struct AppState {
    root: Mutex<Option<PathBuf>>,
    engine: Mutex<Option<SyncEngine>>,
    /// Bumped on every workspace switch; a watcher thread exits when the
    /// generation it was born with is no longer current.
    watch_gen: Arc<AtomicUsize>,
}

fn root_of(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .root
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "no workspace open".to_string())
}

/// The renderer snapshot. While merge conflicts exist the on-disk files carry
/// markers and do not parse — render the OURS side (spec/git-and-diff.md).
/// While a file is stale from ordinary editing, the engine serves the last
/// valid model (ADR-0008) with the live diagnostics riding along.
#[tauri::command]
fn workspace_snapshot(state: State<AppState>) -> Result<serde_json::Value, String> {
    let root = root_of(&state)?;
    if let Some(ctx) = GitContext::discover(&root) {
        if let Ok(Some(snap)) = ctx.ours_snapshot(&root) {
            return serde_json::to_value(&snap).map_err(|e| e.to_string());
        }
    }
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    // Badge lagging L4 facts (spec/l4-introspection.md): a digest recompute,
    // cheap enough to run on every snapshot.
    if let Some(repo) = blastradius_core::introspect::find_repo_root(&root) {
        blastradius_core::introspect::mark_stale(&repo, &mut engine.model);
    }
    let vfs = blastradius_core::vfs::DiskVfs::new(&root);
    let snap = blastradius_core::snapshot::snapshot(&vfs, &engine.model, &engine.diagnostics);
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

/// Editing surface state: staleness, undo/redo availability, editable files.
#[tauri::command]
fn sync_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    let (labels, cursor) = engine.history_labels();
    Ok(serde_json::json!({
        "stale": engine.stale.iter().collect::<Vec<_>>(),
        "staleModel": engine.stale_model(),
        "staleViewIds": engine.stale_view_ids(),
        "canUndo": cursor > 0,
        "canRedo": cursor < labels.len(),
        "undoLabel": (cursor > 0).then(|| labels[cursor - 1].clone()),
        "redoLabel": (cursor < labels.len()).then(|| labels[cursor].clone()),
        "files": engine.editable_files(),
    }))
}

#[tauri::command]
fn apply_operation(state: State<AppState>, op: serde_json::Value) -> Result<serde_json::Value, String> {
    let op: Operation = serde_json::from_value(op).map_err(|e| format!("bad operation: {e}"))?;
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    let tx = engine.apply(op)?;
    Ok(serde_json::json!({ "label": tx.label }))
}

/// Several operations as one transaction, so one undo takes all of them back.
/// The canvas needs this for the first drag in a view: pinning one node makes
/// every other node move, so the drag also pins where the others already are
/// (core::sync::apply_batch).
#[tauri::command]
fn apply_operations(
    state: State<AppState>,
    ops: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let ops: Vec<Operation> = ops
        .into_iter()
        .map(|o| serde_json::from_value(o).map_err(|e| format!("bad operation: {e}")))
        .collect::<Result<_, _>>()?;
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    let tx = engine.apply_batch(ops)?;
    Ok(serde_json::json!({ "label": tx.label }))
}

#[tauri::command]
fn undo_op(state: State<AppState>) -> Result<Option<String>, String> {
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    engine.undo()
}

#[tauri::command]
fn redo_op(state: State<AppState>) -> Result<Option<String>, String> {
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    engine.redo()
}

#[tauri::command]
fn file_text(state: State<AppState>, rel: String) -> Result<String, String> {
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    engine
        .file_text(&rel)
        .map(str::to_string)
        .ok_or_else(|| format!("{rel}: not a workspace file"))
}

/// YAML panel keystrokes (debounced by the frontend). Returns whether the
/// buffer parses — false means the file is now stale and the canvas froze.
#[tauri::command]
fn buffer_update(state: State<AppState>, rel: String, text: String) -> Result<bool, String> {
    let root = root_of(&state)?;
    let mut guard = state.engine.lock().unwrap();
    let engine = guard.get_or_insert_with(|| SyncEngine::open(&root));
    engine.buffer_update(&rel, &text)
}

#[tauri::command]
fn workspace_root(state: State<AppState>) -> Option<String> {
    state.root.lock().unwrap().as_ref().map(|p| p.display().to_string())
}

// ---- onboarding (Phase 5): switch workspaces at runtime ---------------------

/// Point the app at a workspace folder: drop the old engine, retire the old
/// watcher, start a fresh one. The frontend reloads everything afterwards.
fn open_root(app: &tauri::AppHandle, state: &State<AppState>, root: PathBuf) -> Result<String, String> {
    let root = root.canonicalize().unwrap_or(root);
    if !blastradius_core::discover::is_workspace_dir(&root) {
        return Err(format!(
            "{}: not a Blastradius workspace (no blastradius.yaml)",
            root.display()
        ));
    }
    *state.engine.lock().unwrap() = None;
    *state.root.lock().unwrap() = Some(root.clone());
    let gen = state.watch_gen.fetch_add(1, Ordering::SeqCst) + 1;
    spawn_watcher(app.clone(), root.clone(), state.watch_gen.clone(), gen);
    Ok(root.display().to_string())
}

/// Open a folder. A repo root works too: discovery finds the workspace(s)
/// below it — one hit opens directly, several come back as `candidates` for
/// the frontend's picker (ADR-0014).
#[tauri::command]
fn workspace_open(
    app: tauri::AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<serde_json::Value, String> {
    let root = PathBuf::from(&path);
    let root = root.canonicalize().unwrap_or(root);
    if !blastradius_core::discover::is_workspace_dir(&root) {
        let hits = blastradius_core::discover::discover_workspaces(&root);
        match hits.as_slice() {
            // Nothing here. Not an error: pointing the app at your own
            // repository is the expected first move, and answering it with
            // "not a Blastradius workspace" was the whole first-run
            // experience for the first outside user (docs/roadmap.md).
            [] => {
                return Ok(serde_json::json!({
                    "empty": root.display().to_string(),
                    "git": blastradius_core::onboard::git_root(&root).is_some(),
                    // Recommended, not imposed — the dialog offers it as an
                    // editable default and `.` is a valid answer.
                    "suggest": blastradius_core::scaffold::suggested_location(&root),
                }));
            }
            [one] => {
                return open_root(&app, &state, one.clone())
                    .map(|p| serde_json::json!({ "opened": p }));
            }
            many => {
                let candidates: Vec<String> =
                    many.iter().map(|p| p.display().to_string()).collect();
                return Ok(serde_json::json!({ "candidates": candidates }));
            }
        }
    }
    open_root(&app, &state, root).map(|p| serde_json::json!({ "opened": p }))
}

/// Native folder picker. Async so the modal dialog never blocks the IPC
/// thread; rfd handles per-platform quirks (COM on Windows, GTK on Linux).
#[tauri::command(async)]
fn pick_folder() -> Option<String> {
    rfd::FileDialog::new().pick_folder().map(|p| p.display().to_string())
}

/// Where the CLI lives relative to this app, for the MCP registration we
/// write on the user's behalf. A Store install puts `blastradius` on PATH via
/// its execution alias, but a portable install is on no PATH at all — and an
/// MCP server that cannot start is indistinguishable from one that was never
/// registered, which is a bad way to spend a first run.
fn cli_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let cli = exe.with_file_name(if cfg!(windows) { "blastradius.exe" } else { "blastradius" });
    cli.is_file().then(|| cli.display().to_string())
}

/// Which agent integrations to write, mirroring `blastradius init --agents
/// <names> --skills <names>`. Empty lists mean "none of that part".
#[derive(serde::Deserialize, Default)]
struct AgentSetup {
    #[serde(default)]
    mcp: Vec<String>,
    #[serde(default)]
    skills: Vec<String>,
}

/// Scaffold a starter workspace into a folder and open it. A folder that
/// already is a workspace is simply opened.
///
/// Existing files are kept, never overwritten and never fatal — see
/// `scaffold::scaffold_into`.
///
/// `agents` additionally registers the MCP server and writes the skill files,
/// which is what turns an empty workspace into one an agent can actually
/// fill — the CLI has had `init --agents` since 0.2.0 and the app had no way
/// to offer it. What was written comes back so the frontend can say what
/// happened rather than claiming success in the abstract.
#[tauri::command]
fn workspace_init(
    app: tauri::AppHandle,
    state: State<AppState>,
    path: String,
    location: Option<String>,
    agents: Option<AgentSetup>,
) -> Result<serde_json::Value, String> {
    let project = PathBuf::from(&path);
    let project = project.canonicalize().unwrap_or(project);
    // Where inside the project the workspace goes: the model is documentation
    // and belongs with the docs, not scattered through a repository root.
    let location = location.unwrap_or_else(|| ".".into());
    let location = location.trim().to_string();
    blastradius_core::scaffold::check_location(&location)?;
    let root = if location == "." { project.clone() } else { project.join(&location) };
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let root = root.canonicalize().unwrap_or(root);

    let mut created: Vec<String> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    let scaffolded = !blastradius_core::discover::is_workspace_dir(&root);
    if scaffolded {
        // Named after the project, not after the docs folder it sits in.
        let name = blastradius_core::scaffold::name_for(&project);
        let done = blastradius_core::scaffold::scaffold_into(&root, &name)?;
        created = done.created;
        kept = done.skipped;
    }
    let agents = agents.unwrap_or_default();
    let mut log: Vec<String> = Vec::new();
    // The hand-off depends on what was actually set up — a workflow to invoke,
    // MCP tools to call, or neither — so it is built from the same selections
    // rather than being one fixed sentence.
    let prompt = blastradius_core::onboard::sample_prompt(
        &blastradius_core::onboard::workspace_rel(&root),
        &agents.skills,
        &agents.mcp,
    );
    let workflows = blastradius_core::onboard::workflow_summary(&agents.skills);
    if !agents.mcp.is_empty() || !agents.skills.is_empty() {
        log = blastradius_core::onboard::setup(
            &root,
            &blastradius_core::onboard::SetupOptions {
                git_init: false, // the user's repository is the user's to create
                mcp: agents.mcp,
                skills: agents.skills,
                command: cli_command(),
            },
        );
    }
    let opened = open_root(&app, &state, root)?;
    Ok(serde_json::json!({
        "opened": opened, "scaffolded": scaffolded,
        "created": created, "kept": kept,
        "log": log, "prompt": prompt, "workflows": workflows,
    }))
}

/// A throwaway sample workspace under the OS temp dir — the "try it before
/// pointing it at your repo" path. Fully editable; recreated when absent.
#[tauri::command]
fn workspace_demo(app: tauri::AppHandle, state: State<AppState>) -> Result<String, String> {
    let dir = std::env::temp_dir().join("blastradius-demo");
    if !blastradius_core::discover::is_workspace_dir(&dir) {
        for (rel, text) in blastradius_core::scaffold::starter_workspace("Acme Payments") {
            let target = dir.join(&rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, text).map_err(|e| e.to_string())?;
        }
    }
    open_root(&app, &state, dir)
}

#[tauri::command]
fn git_status(state: State<AppState>) -> Result<Option<serde_json::Value>, String> {
    let root = root_of(&state)?;
    let Some(ctx) = GitContext::discover(&root) else {
        return Ok(None);
    };
    let status = ctx.status().map_err(|e| e.to_string())?;
    Ok(Some(serde_json::to_value(&status).map_err(|e| e.to_string())?))
}

#[tauri::command]
fn git_diff(state: State<AppState>, base: Option<String>) -> Result<Option<serde_json::Value>, String> {
    let root = root_of(&state)?;
    let Some(ctx) = GitContext::discover(&root) else {
        return Ok(None);
    };
    let base_label = match base.or_else(|| ctx.default_base()) {
        Some(b) => b,
        None => return Ok(None),
    };
    let (base_ws, base_diags) = ctx.load_at(&base_label)?;
    if blastradius_core::diagnostics::has_errors(&base_diags) {
        return Err(format!("base revision {base_label} does not parse"));
    }
    let (cur_ws, cur_diags) = blastradius_core::load_workspace(&root);
    if blastradius_core::diagnostics::has_errors(&cur_diags) {
        return Err("working tree does not parse".to_string());
    }
    let payload = blastradius_core::diff::diff_payload(&base_label, &base_ws, &cur_ws);
    Ok(Some(serde_json::to_value(&payload).map_err(|e| e.to_string())?))
}

#[tauri::command]
fn git_history(state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let root = root_of(&state)?;
    let Some(ctx) = GitContext::discover(&root) else {
        return Ok(Vec::new());
    };
    let commits = ctx.history(200)?;
    commits
        .iter()
        .map(|c| serde_json::to_value(c).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
fn snapshot_at(state: State<AppState>, refspec: String) -> Result<serde_json::Value, String> {
    let root = root_of(&state)?;
    let ctx = GitContext::discover(&root).ok_or("not a git repository")?;
    let snap = ctx.snapshot_at(&refspec)?;
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

#[tauri::command]
fn git_conflicts(state: State<AppState>) -> Result<Option<serde_json::Value>, String> {
    let root = root_of(&state)?;
    let Some(ctx) = GitContext::discover(&root) else {
        return Ok(None);
    };
    match ctx.conflicts(&root)? {
        Some(c) => Ok(Some(serde_json::to_value(&c).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

/// Apply per-element ours/theirs decisions to the current merge conflict
/// (ADR-0015): rebuilt from the chosen side's stage text via CST splices,
/// validated, written, and staged through the user's own git binary.
#[tauri::command]
fn resolve_conflicts(
    state: State<AppState>,
    resolution: serde_json::Value,
) -> Result<Vec<String>, String> {
    let root = root_of(&state)?;
    let ctx = GitContext::discover(&root).ok_or("not inside a git repository")?;
    let res: blastradius_core::resolve::Resolution =
        serde_json::from_value(resolution).map_err(|e| format!("bad resolution: {e}"))?;
    blastradius_core::resolve::resolve(&ctx, &root, &res)
}

#[tauri::command]
fn open_in_editor(state: State<AppState>, rel: String) -> Result<(), String> {
    let root = root_of(&state)?;
    open_path(&root, &rel, "workspace")
}

/// Open a repo-root-relative source file — derived (L4) elements point at
/// code outside the workspace directory (spec/l4-introspection.md).
#[tauri::command]
fn open_source(state: State<AppState>, rel: String) -> Result<(), String> {
    let root = root_of(&state)?;
    let repo = blastradius_core::introspect::find_repo_root(&root)
        .ok_or_else(|| "no repository root above the workspace".to_string())?;
    open_path(&repo, &rel, "repository")
}

fn open_path(base: &std::path::Path, rel: &str, scope: &str) -> Result<(), String> {
    if rel.contains("..") {
        return Err(format!("path escapes the {scope}"));
    }
    let path = base.join(rel);
    if !path.is_file() {
        return Err(format!("{rel}: not a file"));
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd").args(["/c", "start", ""]).arg(&path).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(&path).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Share: build the self-contained HTML (ADR-0009) and save it to Downloads.
#[tauri::command]
fn export_html(state: State<AppState>, with_bodies: bool) -> Result<String, String> {
    let root = root_of(&state)?;
    let (ws, diags) = blastradius_core::load_workspace(&root);
    if blastradius_core::diagnostics::has_errors(&diags) {
        return Err("workspace is invalid — fix errors before exporting".into());
    }
    let vfs = blastradius_core::vfs::DiskVfs::new(&root);
    let options = blastradius_core::export::ExportOptions { include_doc_bodies: with_bodies };
    let html = blastradius_core::export::export_html(&vfs, &ws, &diags, &options)?;
    let name = format!("{}-architecture.html", slug(&ws.name));
    let path = downloads_path(&name)?;
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Save a frontend-produced export (SVG text or PNG base64) to Downloads.
#[tauri::command]
fn save_export(name: String, data: String, base64: bool) -> Result<String, String> {
    if name.contains(['/', '\\']) || name.contains("..") {
        return Err("bad file name".into());
    }
    let path = downloads_path(&name)?;
    if base64 {
        let bytes = decode_base64(&data)?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
    }
    Ok(path.display().to_string())
}

fn downloads_path(name: &str) -> Result<PathBuf, String> {
    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or("no downloads directory")?;
    Ok(dir.join(name))
}

fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for c in s.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' {
            continue;
        }
        let v = T.iter().position(|&t| t == c).ok_or("bad base64")? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn startup_root() -> Option<PathBuf> {
    let candidate = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| Some(std::env::current_dir().ok()?.join("docs")))?;
    let root = candidate.canonicalize().unwrap_or(candidate);
    blastradius_core::discover::is_workspace_dir(&root).then_some(root)
}

fn main() {
    let root = startup_root();

    tauri::Builder::default()
        .manage(AppState {
            root: Mutex::new(root.clone()),
            engine: Mutex::new(None),
            watch_gen: Arc::new(AtomicUsize::new(0)),
        })
        .invoke_handler(tauri::generate_handler![
            workspace_snapshot,
            workspace_root,
            sync_status,
            apply_operation,
            apply_operations,
            undo_op,
            redo_op,
            file_text,
            buffer_update,
            git_status,
            git_diff,
            git_history,
            snapshot_at,
            git_conflicts,
            resolve_conflicts,
            open_in_editor,
            open_source,
            export_html,
            save_export,
            workspace_open,
            workspace_init,
            workspace_demo,
            pick_folder
        ])
        .setup(move |app| {
            if let Some(root) = root {
                let state: State<AppState> = app.state();
                let watch_gen = state.watch_gen.clone();
                spawn_watcher(app.handle().clone(), root, watch_gen, 0);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Debounced watcher. The engine's external_scan compares disk to its cache —
/// our own writes match and produce no event (the echo-loop killer, now real).
fn spawn_watcher(app: tauri::AppHandle, root: PathBuf, watch_gen: Arc<AtomicUsize>, my_gen: usize) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut watcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let event: notify::Event = event;
                if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("watcher unavailable: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&root, notify::RecursiveMode::Recursive) {
            eprintln!("cannot watch {}: {e}", root.display());
            return;
        }
        if let Some(git_dir) = git2::Repository::discover(&root).ok().map(|r| r.path().to_path_buf())
        {
            let _ = watcher.watch(&git_dir.join("HEAD"), notify::RecursiveMode::NonRecursive);
            let _ = watcher.watch(&git_dir.join("index"), notify::RecursiveMode::NonRecursive);
        }
        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(()) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // idle poll: only here to notice workspace switches
                    if watch_gen.load(Ordering::SeqCst) != my_gen {
                        return;
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
            while rx.recv_timeout(std::time::Duration::from_millis(150)).is_ok() {}
            // a retired watcher (workspace switched away) must not touch state
            if watch_gen.load(Ordering::SeqCst) != my_gen {
                return;
            }
            // Ask the engine whether anything really changed; git metadata
            // (HEAD/index) always refreshes the chrome.
            let changed = {
                let state: State<AppState> = app.state();
                let mut guard = state.engine.lock().unwrap();
                match guard.as_mut() {
                    Some(engine) => engine.external_scan(),
                    None => true,
                }
            };
            let _ = app.emit("workspace-changed", changed);
        }
    });
}
