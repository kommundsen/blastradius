//! Blastradius desktop shell. Phase 1: read-only — the WebView renders, the
//! Core loads and watches; there is no write path anywhere in this binary.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use notify::Watcher;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, State};

/// The open workspace. Phase 1 opens exactly one, chosen at startup.
struct AppState {
    root: Mutex<Option<PathBuf>>,
}

/// Serialized snapshot for the WebView. Loading is cheap enough to redo per
/// request (Phase 0 budget: <50ms for 500 elements), which keeps the shell
/// stateless — the files are the truth (ADR-0008), so there is nothing to cache
/// that could go stale.
#[tauri::command]
fn workspace_snapshot(state: State<AppState>) -> Result<serde_json::Value, String> {
    let root = state
        .root
        .lock()
        .unwrap()
        .clone()
        .ok_or("no workspace open")?;
    let (ws, diags) = blastradius_core::load_workspace(&root);
    let snap = blastradius_core::snapshot::snapshot(&root, &ws, &diags);
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

#[tauri::command]
fn workspace_root(state: State<AppState>) -> Option<String> {
    state
        .root
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.display().to_string())
}

/// Workspace to open: first CLI arg, else `docs/` under the current directory
/// (the dogfood default), else none — the UI shows how to launch.
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
        .invoke_handler(tauri::generate_handler![workspace_snapshot, workspace_root])
        .setup(move |app| {
            if let Some(root) = root {
                spawn_watcher(app.handle().clone(), root);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Watch the workspace; on any change, debounce briefly and tell the WebView
/// to re-request the snapshot. The event carries nothing — the snapshot
/// command is the single read path (one code path for external and internal
/// changes, per ADR-0008's echo-loop rule; trivially so in Phase 1, where
/// there are no internal writes).
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
        // Debounce: editors save in bursts (write + rename + metadata).
        while rx.recv().is_ok() {
            while rx.recv_timeout(std::time::Duration::from_millis(150)).is_ok() {}
            let _ = app.emit("workspace-changed", ());
        }
    });
}
