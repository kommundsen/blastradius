//! Performance budgets (spec/sync-engine.md), enforced against a generated
//! ~510-element benchmark workspace. Budgets are release-build contracts:
//! under `debug_assertions` these tests are ignored (a debug build is
//! legitimately 10-20x slower). CI runs them with `--release`.
//!
//! Budget composition: "keystroke → canvas < 250ms" spans two processes. The
//! core share (parse + validate after a buffer edit) is enforced here at
//! 150ms; the render share (< 100ms: layout + DOM on the current view) is
//! enforced in WebKit by ui/tests/e2e/perf.spec.mjs.

use blastradius_core::scaffold::benchmark_workspace;
use blastradius_core::sync::{Operation, SyncEngine};
use std::path::PathBuf;
use std::time::Instant;

fn materialize(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blastradius-bench-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (rel, text) in benchmark_workspace(20) {
        let path = dir.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    if let Some(j) = blastradius_core::sync::journal_path(&dir) {
        let _ = std::fs::remove_file(j);
    }
    dir
}

/// Best-of-N wall time in milliseconds — the budget bounds the machine's
/// capability, not scheduler noise.
fn best_of<F: FnMut()>(n: usize, mut f: F) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

#[test]
#[cfg_attr(debug_assertions, ignore = "budgets are release-build contracts (CI runs --release)")]
fn parse_and_validate_500_elements_under_50ms() {
    let dir = materialize("parse");
    let (ws, diags) = blastradius_core::load_workspace(&dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags), "{diags:?}");
    assert!(ws.elements.len() >= 500, "benchmark shrank: {} elements", ws.elements.len());

    let ms = best_of(5, || {
        let _ = blastradius_core::load_workspace(&dir);
    });
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("parse+validate {} elements: {ms:.1}ms (budget 50ms)", ws.elements.len());
    assert!(ms < 50.0, "parse+validate budget blown: {ms:.1}ms >= 50ms");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "budgets are release-build contracts (CI runs --release)")]
fn canvas_drop_to_file_write_under_30ms() {
    let dir = materialize("pin");
    let mut engine = SyncEngine::open(&dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);

    let mut y = 4;
    let ms = best_of(5, || {
        engine
            .apply(Operation::Pin {
                view: Some("sys-0-l2".into()),
                level: "L2".into(),
                scope: Some("sys-0".into()),
                id: "sys-0.svc-2".into(),
                x: 4,
                y,
            })
            .unwrap();
        y += 1; // each drop lands somewhere new, like a real drag
    });
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("canvas drop -> file write: {ms:.1}ms (budget 30ms)");
    assert!(ms < 30.0, "drop-to-write budget blown: {ms:.1}ms >= 30ms");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "budgets are release-build contracts (CI runs --release)")]
fn keystroke_core_share_under_150ms() {
    let dir = materialize("keystroke");
    let mut engine = SyncEngine::open(&dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);
    let base = engine.file_text("model/sys-0.yaml").unwrap().to_string();

    let mut i = 0;
    let ms = best_of(5, || {
        let text = base.replace("name: System 0", &format!("name: System {i}x"));
        engine.buffer_update("model/sys-0.yaml", &text).unwrap();
        i += 1;
    });
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("keystroke core share (write + reparse + validate): {ms:.1}ms (budget 150ms)");
    assert!(ms < 150.0, "keystroke core-share budget blown: {ms:.1}ms >= 150ms");
}
