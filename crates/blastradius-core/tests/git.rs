//! Git service tests (ADR-0007, spec/git-and-diff.md): revision loading,
//! semantic diff between commits, history filtering, and conflict-side
//! parsing — all against throwaway repos built in a temp dir, no fixtures on
//! the real repository.

use blastradius_core::git::GitContext;
use git2::{Repository, Signature};
use std::fs;
use std::path::{Path, PathBuf};

struct TempRepo {
    dir: PathBuf,
    repo: Repository,
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn sig() -> Signature<'static> {
    Signature::now("test", "test@example.com").unwrap()
}

fn temp_repo(name: &str) -> TempRepo {
    let dir = std::env::temp_dir().join(format!("blastradius-git-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let repo = Repository::init(&dir).unwrap();
    TempRepo { dir, repo }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn commit_all(t: &TempRepo, msg: &str) -> git2::Oid {
    let mut index = t.repo.index().unwrap();
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
    index.write().unwrap();
    let tree = t.repo.find_tree(index.write_tree().unwrap()).unwrap();
    let parent = t.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    t.repo.commit(Some("HEAD"), &sig(), &sig(), msg, &tree, &parents).unwrap()
}

const MANIFEST: &str = "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

fn shop_v1() -> &'static str {
    "system: shop\ncontainers:\n  web:\n    name: Web App\n    tech: React\n  api:\n    name: API\n    tech: Go\nrelations:\n  - from: web\n    to: api\n    label: calls\n    protocol: HTTPS\n"
}

fn shop_v2() -> &'static str {
    // web renamed, api retech, cache added, protocol changed
    "system: shop\ncontainers:\n  web:\n    name: Storefront\n    tech: React\n  api:\n    name: API\n    tech: Rust\n  cache:\n    name: Cache\n    tech: Redis\nrelations:\n  - from: web\n    to: api\n    label: calls\n    protocol: gRPC\n  - from: api\n    to: cache\n    label: reads\n"
}

#[test]
fn revision_loading_and_semantic_diff_between_commits() {
    let t = temp_repo("diff");
    // workspace lives in a subfolder — exercises the prefix path
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    write(&t.dir, "docs/model/shop.yaml", shop_v1());
    let c1 = commit_all(&t, "v1");
    write(&t.dir, "docs/model/shop.yaml", shop_v2());
    let c2 = commit_all(&t, "v2");

    let ctx = GitContext::discover(&t.dir.join("docs")).expect("repo discovered");

    // load both revisions purely from the object database
    let (ws1, d1) = ctx.load_at(&c1.to_string()).unwrap();
    let (ws2, d2) = ctx.load_at(&c2.to_string()).unwrap();
    assert!(!blastradius_core::diagnostics::has_errors(&d1));
    assert!(!blastradius_core::diagnostics::has_errors(&d2));
    assert_eq!(ws1.elements.len(), 3); // shop, web, api
    assert_eq!(ws2.elements.len(), 4); // + cache

    let payload = blastradius_core::diff::diff_payload("base", &ws1, &ws2);
    let changes: std::collections::BTreeMap<_, _> =
        payload.elements.iter().map(|e| (e.id.as_str(), e.change)).collect();
    assert_eq!(changes.get("shop.web"), Some(&"changed"), "rename = changed, never add+remove");
    assert_eq!(changes.get("shop.api"), Some(&"changed"));
    assert_eq!(changes.get("shop.cache"), Some(&"added"));
    assert_eq!(changes.get("shop"), None, "untouched parent not in diff");

    let rels: std::collections::BTreeMap<_, _> = payload
        .relations
        .iter()
        .map(|r| (format!("{}->{}", r.from, r.to), r.change))
        .collect();
    // endpoints resolved to full ids for the renderer
    assert_eq!(rels.get("shop.web->shop.api"), Some(&"changed"), "protocol change");
    assert_eq!(rels.get("shop.api->shop.cache"), Some(&"added"));

    // removed elements carry ghost data: diff v2 -> v1 removes cache
    let back = blastradius_core::diff::diff_payload("b", &ws2, &ws1);
    let cache = back.elements.iter().find(|e| e.id == "shop.cache").unwrap();
    assert_eq!(cache.change, "removed");
    assert_eq!(cache.element.name, "Cache", "ghost keeps base-side element data");
}

#[test]
fn history_lists_only_workspace_commits() {
    let t = temp_repo("history");
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    write(&t.dir, "docs/model/shop.yaml", shop_v1());
    commit_all(&t, "workspace v1");
    write(&t.dir, "src/other.txt", "unrelated");
    commit_all(&t, "unrelated change");
    write(&t.dir, "docs/model/shop.yaml", shop_v2());
    commit_all(&t, "workspace v2");

    let ctx = GitContext::discover(&t.dir.join("docs")).unwrap();
    let history = ctx.history(50).unwrap();
    let summaries: Vec<&str> = history.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, vec!["workspace v2", "workspace v1"], "path-filtered, newest first");
}

#[test]
fn status_reports_branch_and_dirty_workspace_files() {
    let t = temp_repo("status");
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    write(&t.dir, "docs/model/shop.yaml", shop_v1());
    commit_all(&t, "v1");

    let ctx = GitContext::discover(&t.dir.join("docs")).unwrap();
    let clean = ctx.status().unwrap();
    assert_eq!(clean.dirty, 0);
    assert!(clean.conflicted.is_empty());

    write(&t.dir, "docs/model/shop.yaml", shop_v2());
    write(&t.dir, "unrelated.txt", "outside workspace");
    let dirty = ctx.status().unwrap();
    assert_eq!(dirty.dirty, 1, "only workspace files count");
}

#[test]
fn conflict_sides_parse_and_differ() {
    let t = temp_repo("conflict");
    write(&t.dir, "blastradius.yaml", MANIFEST); // workspace at repo root: prefix = ""
    write(&t.dir, "model/shop.yaml", shop_v1());
    let base = commit_all(&t, "base");

    // branch A: rename web
    let ours = shop_v1().replace("name: Web App", "name: Storefront");
    write(&t.dir, "model/shop.yaml", &ours);
    let ours_commit = commit_all(&t, "ours");

    // branch B from base: different rename of the same line
    let base_commit = t.repo.find_commit(base).unwrap();
    t.repo.branch("theirs", &base_commit, false).unwrap();
    t.repo.set_head("refs/heads/theirs").unwrap();
    t.repo
        .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    let theirs = shop_v1().replace("name: Web App", "name: Shop Frontend");
    write(&t.dir, "model/shop.yaml", &theirs);
    commit_all(&t, "theirs");

    // merge ours into theirs -> conflict
    let ours_c = t.repo.find_commit(ours_commit).unwrap();
    let ours_ac = t.repo.find_annotated_commit(ours_commit).unwrap();
    let _ = &ours_c;
    t.repo.merge(&[&ours_ac], None, None).unwrap();
    assert!(t.repo.index().unwrap().has_conflicts(), "merge must conflict");

    let ctx = GitContext::discover(&t.dir).unwrap();
    let status = ctx.status().unwrap();
    assert_eq!(status.conflicted, vec!["model/shop.yaml"]);

    let conflicts = ctx.conflicts(&t.dir).unwrap().expect("conflicts present");
    assert_eq!(conflicts.files, vec!["model/shop.yaml"]);
    let web = conflicts.elements.iter().find(|e| e.id == "shop.web").expect("web conflicted");
    // HEAD is `theirs` branch, so index stage-2 (ours) = Shop Frontend
    let side_names: Vec<&str> = [&web.ours, &web.theirs]
        .iter()
        .filter_map(|s| s.as_ref().map(|c| c.name.as_str()))
        .collect();
    assert!(side_names.contains(&"Storefront"));
    assert!(side_names.contains(&"Shop Frontend"));
    // api untouched on both sides -> not conflicted
    assert!(!conflicts.elements.iter().any(|e| e.id == "shop.api"));

    // while conflicted, the model stays viewable via the ours overlay
    let snap = ctx.ours_snapshot(&t.dir).unwrap().expect("ours snapshot during conflict");
    assert!(snap.diagnostics.iter().all(|d| d.severity != "error"), "ours side parses clean");
    let web = snap.elements.iter().find(|e| e.id == "shop.web").unwrap();
    assert_eq!(web.name, "Shop Frontend", "renders HEAD side (ours = current branch)");

    // resolve round-trip: write merged content, stage it — conflicts clear
    let resolved = shop_v1().replace("name: Web App", "name: Storefront");
    write(&t.dir, "model/shop.yaml", &resolved);
    let mut index = t.repo.index().unwrap();
    index.add_path(Path::new("model/shop.yaml")).unwrap();
    index.write().unwrap();
    assert!(ctx.conflicts(&t.dir).unwrap().is_none(), "resolution clears conflict state");
}
