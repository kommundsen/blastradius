//! Blastradius desktop shell. Phase 3: the sync engine arrives — canvas and
//! YAML panel propose transactions; files stay the single source of truth
//! (ADR-0008). Git remains read-only (ADR-0007).

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use blastradius_core::git::GitContext;
use blastradius_core::sync::{Operation, SyncEngine};
use notify::Watcher;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

struct AppState {
    root: Mutex<Option<PathBuf>>,
    engine: Mutex<Option<SyncEngine>>,
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

#[tauri::command]
fn open_in_editor(state: State<AppState>, rel: String) -> Result<(), String> {
    let root = root_of(&state)?;
    if rel.contains("..") {
        return Err("path escapes the workspace".to_string());
    }
    let path = root.join(&rel);
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

fn startup_root() -> Option<PathBuf> {
    let candidate = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| Some(std::env::current_dir().ok()?.join("docs")))?;
    let root = candidate.canonicalize().unwrap_or(candidate);
    root.join("workspace.yaml").is_file().then_some(root)
}

fn main() {
    let root = startup_root();

    tauri::Builder::default()
        .manage(AppState { root: Mutex::new(root.clone()), engine: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            workspace_snapshot,
            workspace_root,
            sync_status,
            apply_operation,
            undo_op,
            redo_op,
            file_text,
            buffer_update,
            git_status,
            git_diff,
            git_history,
            snapshot_at,
            git_conflicts,
            open_in_editor
        ])
        .setup(move |app| {
            if let Some(root) = root {
                spawn_watcher(app.handle().clone(), root);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Debounced watcher. The engine's external_scan compares disk to its cache —
/// our own writes match and produce no event (the echo-loop killer, now real).
fn spawn_watcher(app: tauri::AppHandle, root: PathBuf) {
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
        while rx.recv().is_ok() {
            while rx.recv_timeout(std::time::Duration::from_millis(150)).is_ok() {}
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
