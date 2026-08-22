//! Blastradius desktop shell. Phase 2: read-only rendering + git awareness.
//! The WebView renders; the Core loads, watches, and reads the repository.
//! Still no write path: git stays read-only (ADR-0007), files are edited by
//! the user's own tooling.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use blastradius_core::git::GitContext;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, State};

struct AppState {
    root: Mutex<Option<PathBuf>>,
}

fn root_of(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .root
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "no workspace open".to_string())
}

/// Loading is cheap enough to redo per request (Phase 0 budget), which keeps
/// the shell stateless — the files are the truth (ADR-0008), so there is
/// nothing to cache that could go stale. The same goes for the GitContext:
/// rediscovered per call, so external `git checkout`/`merge` are always seen.
#[tauri::command]
fn workspace_snapshot(state: State<AppState>) -> Result<serde_json::Value, String> {
    let root = root_of(&state)?;
    // During a merge conflict the on-disk files carry markers and do not
    // parse; the model must stay viewable, so render the OURS side
    // (spec/git-and-diff.md) — the conflict chrome tells the user why.
    if let Some(ctx) = GitContext::discover(&root) {
        if let Ok(Some(snap)) = ctx.ours_snapshot(&root) {
            return serde_json::to_value(&snap).map_err(|e| e.to_string());
        }
    }
    let (ws, diags) = blastradius_core::load_workspace(&root);
    let vfs = blastradius_core::vfs::DiskVfs::new(&root);
    let snap = blastradius_core::snapshot::snapshot(&vfs, &ws, &diags);
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

#[tauri::command]
fn workspace_root(state: State<AppState>) -> Option<String> {
    state.root.lock().unwrap().as_ref().map(|p| p.display().to_string())
}

/// Branch, dirty count, ahead/behind, conflicted files — or null outside a
/// repository (git surfaces are absent, not errors: ADR-0007).
#[tauri::command]
fn git_status(state: State<AppState>) -> Result<Option<serde_json::Value>, String> {
    let root = root_of(&state)?;
    let Some(ctx) = GitContext::discover(&root) else {
        return Ok(None);
    };
    let status = ctx.status().map_err(|e| e.to_string())?;
    Ok(Some(serde_json::to_value(&status).map_err(|e| e.to_string())?))
}

/// Semantic diff of the working tree against a base revision (default: the
/// merge-base with the default branch). Layout changes ride along separately,
/// for the toggle (spec/git-and-diff.md).
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

/// Commits touching workspace files, newest first — the History control.
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

/// Time-travel: the full snapshot at a revision, read-only.
#[tauri::command]
fn snapshot_at(state: State<AppState>, refspec: String) -> Result<serde_json::Value, String> {
    let root = root_of(&state)?;
    let ctx = GitContext::discover(&root).ok_or("not a git repository")?;
    let snap = ctx.snapshot_at(&refspec)?;
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

/// Ours/theirs element values during a merge conflict, or null when clean.
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

/// "Resolve in editor": hand the conflicted file to the OS default handler.
/// The watcher sees the resolution — the app itself never writes (ADR-0007).
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
        .manage(AppState { root: Mutex::new(root.clone()) })
        .invoke_handler(tauri::generate_handler![
            workspace_snapshot,
            workspace_root,
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

/// Watch the workspace AND the repository metadata: commits, branch switches
/// and merges change what the git surfaces show without touching workspace
/// files. One debounced event either way; the WebView re-requests everything.
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
        // .git/HEAD + index: branch switches, commits, merge state.
        if let Some(ctx_dir) = GitContext::discover(&root)
            .and(Some(root.clone()))
            .and_then(|r| git_dir_of(&r))
        {
            let _ = watcher.watch(&ctx_dir.join("HEAD"), notify::RecursiveMode::NonRecursive);
            let _ = watcher.watch(&ctx_dir.join("index"), notify::RecursiveMode::NonRecursive);
        }
        while rx.recv().is_ok() {
            while rx.recv_timeout(std::time::Duration::from_millis(150)).is_ok() {}
            let _ = app.emit("workspace-changed", ());
        }
    });
}

fn git_dir_of(root: &PathBuf) -> Option<PathBuf> {
    let repo = git2::Repository::discover(root).ok()?;
    Some(repo.path().to_path_buf())
}
