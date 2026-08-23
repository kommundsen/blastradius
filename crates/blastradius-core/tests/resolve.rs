//! In-app conflict resolution (0.2.0 theme 3, ADR-0015) — the exit
//! criterion lives here: a manufactured merge conflict resolves entirely
//! through the resolver and ends byte-clean — files valid, comments and
//! formatting intact, index conflict-free, resolution staged.

use blastradius_core::git::GitContext;
use blastradius_core::resolve::{resolve, Resolution, Side};
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
    let dir =
        std::env::temp_dir().join(format!("blastradius-resolve-{name}-{}", std::process::id()));
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

const MANIFEST: &str = "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\n";

// a comment sits above `web` on purpose: resolutions must preserve it
const SHOP: &str = "system: shop\ncontainers:\n  # the storefront — reviewed quarterly\n  web:\n    name: Web App\n    tech: React\n  api:\n    name: API\n    tech: Go\n";

/// Commit base, commit `mine` on master, commit `theirs_text` on a branch
/// from base, then merge master into the branch — index now conflicts, with
/// stage-2 (ours) = `theirs_text` (the checked-out branch) and stage-3 =
/// `mine`. Returns after asserting the conflict exists.
fn manufacture_conflict(t: &TempRepo, mine: &str, branch_text: &str) {
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SHOP);
    let base = commit_all(t, "base");

    write(&t.dir, "model/shop.yaml", mine);
    let mine_commit = commit_all(t, "mine");

    let base_commit = t.repo.find_commit(base).unwrap();
    t.repo.branch("side", &base_commit, false).unwrap();
    t.repo.set_head("refs/heads/side").unwrap();
    t.repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
    write(&t.dir, "model/shop.yaml", branch_text);
    commit_all(t, "side");

    let mine_ac = t.repo.find_annotated_commit(mine_commit).unwrap();
    t.repo.merge(&[&mine_ac], None, None).unwrap();
    assert!(t.repo.index().unwrap().has_conflicts(), "merge must conflict");
}

#[test]
fn per_element_choice_resolves_byte_clean_and_stages() {
    let t = temp_repo("field");
    // both sides rename web differently
    let mine = SHOP.replace("name: Web App", "name: Storefront");
    let side = SHOP.replace("name: Web App", "name: Shop Frontend");
    manufacture_conflict(&t, &mine, &side);

    let ctx = GitContext::discover(&t.dir).unwrap();
    // ours (checked out) says "Shop Frontend"; take the incoming "Storefront"
    let res = Resolution {
        elements: [("shop.web".to_string(), Side::Theirs)].into(),
        ..Default::default()
    };
    let written = resolve(&ctx, &t.dir, &res).unwrap();
    assert_eq!(written, vec!["model/shop.yaml"]);

    // byte-clean: ours base with exactly the chosen field spliced — the
    // comment above `web` survives, everything else untouched
    let expect = side.replace("name: Shop Frontend", "name: Storefront");
    assert_eq!(fs::read_to_string(t.dir.join("model/shop.yaml")).unwrap(), expect);

    // conflict gone, resolution staged, workspace valid
    assert!(!ctx.has_conflicts());
    let (_, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags));
    let mut idx = t.repo.index().unwrap();
    idx.read(true).unwrap(); // the external `git add` rewrote the index file
    assert!(idx.get_path(Path::new("model/shop.yaml"), 0).is_some(), "staged at stage 0");
}

#[test]
fn default_resolution_keeps_ours_verbatim() {
    let t = temp_repo("default");
    let mine = SHOP.replace("name: Web App", "name: Storefront");
    let side = SHOP.replace("name: Web App", "name: Shop Frontend");
    manufacture_conflict(&t, &mine, &side);

    let ctx = GitContext::discover(&t.dir).unwrap();
    resolve(&ctx, &t.dir, &Resolution::default()).unwrap();
    assert_eq!(fs::read_to_string(t.dir.join("model/shop.yaml")).unwrap(), side);
    assert!(!ctx.has_conflicts());
}

#[test]
fn delete_vs_edit_honours_the_deletion_choice() {
    let t = temp_repo("delete");
    // mine deletes api; the branch re-techs it — a classic modify/delete
    let mine = SHOP.replace("  api:\n    name: API\n    tech: Go\n", "");
    let side = SHOP.replace("tech: Go", "tech: Rust");
    manufacture_conflict(&t, &mine, &side);

    let ctx = GitContext::discover(&t.dir).unwrap();
    let res = Resolution {
        elements: [("shop.api".to_string(), Side::Theirs)].into(),
        ..Default::default()
    };
    resolve(&ctx, &t.dir, &res).unwrap();

    // ours base (api re-teched) minus the deleted element = mine's shape,
    // but the untouched comment block stays exactly as ours had it
    let text = fs::read_to_string(t.dir.join("model/shop.yaml")).unwrap();
    assert!(!text.contains("api:"), "{text}");
    assert!(text.contains("# the storefront — reviewed quarterly"));
    assert!(!ctx.has_conflicts());
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags));
    assert!(ws.elements.get("shop.api").is_none());
}

#[test]
fn invalid_resolution_is_refused_before_touching_disk() {
    let t = temp_repo("refuse");
    // mine deletes api but a relation to it arrives from the branch side —
    // taking the deletion while keeping the relation would dangle
    let mine = SHOP.replace("  api:\n    name: API\n    tech: Go\n", "");
    let side = SHOP.to_string() + "relations:\n  - from: web\n    to: api\n    label: calls\n";
    manufacture_conflict(&t, &mine, &side);

    let ctx = GitContext::discover(&t.dir).unwrap();
    let res = Resolution {
        elements: [("shop.api".to_string(), Side::Theirs)].into(),
        ..Default::default()
    };
    let err = resolve(&ctx, &t.dir, &res).unwrap_err();
    assert!(err.contains("resolution would break the workspace"), "{err}");
    // nothing written: the working tree still carries the conflict markers
    let disk = fs::read_to_string(t.dir.join("model/shop.yaml")).unwrap();
    assert!(disk.contains("<<<<<<<"), "working tree untouched on refusal");
    assert!(ctx.has_conflicts());
}
