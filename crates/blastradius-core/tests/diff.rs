//! Semantic diff behaviour (spec/git-and-diff.md): same id = same element,
//! renames are Changed, layout never appears, doc relinks mark elements.

use blastradius_core::diff::{diff, Change};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn semantic_diff_classification() {
    let (base, d1) = blastradius_core::load_workspace(&fixture("valid"));
    let (cur, d2) = blastradius_core::load_workspace(&fixture("changed"));
    assert!(!blastradius_core::diagnostics::has_errors(&d1));
    assert!(!blastradius_core::diagnostics::has_errors(&d2));

    let d = diff(&base, &cur);

    // web renamed (name field) AND doc adr-0001 relinked api->web => Changed
    assert_eq!(d.elements.get("shop.web"), Some(&Change::Changed));
    // api retech Go->Rust AND lost its doc link => Changed (not add/remove)
    assert_eq!(d.elements.get("shop.api"), Some(&Change::Changed));
    // cache is new
    assert_eq!(d.elements.get("shop.cache"), Some(&Change::Added));
    // router component deleted
    assert_eq!(d.elements.get("shop.api.router"), Some(&Change::Removed));
    // untouched elements absent from the diff
    assert_eq!(d.elements.get("user"), None);
    assert_eq!(d.elements.get("mainframe"), None);
    assert_eq!(d.elements.get("shop"), None);

    // relations: protocol HTTPS->gRPC on same (from,to,label) => Changed
    let key = ("web".to_string(), "api".to_string(), "calls".to_string());
    assert_eq!(d.relations.get(&key), Some(&Change::Changed));
    // api->mainframe gone, api->cache new
    let removed = ("api".to_string(), "mainframe".to_string(), "settles".to_string());
    let added = ("api".to_string(), "cache".to_string(), "reads".to_string());
    assert_eq!(d.relations.get(&removed), Some(&Change::Removed));
    assert_eq!(d.relations.get(&added), Some(&Change::Added));
    // user->web untouched
    let same = ("user".to_string(), "web".to_string(), "uses".to_string());
    assert_eq!(d.relations.get(&same), None);
}

#[test]
fn identical_workspaces_diff_empty() {
    let (a, _) = blastradius_core::load_workspace(&fixture("valid"));
    let (b, _) = blastradius_core::load_workspace(&fixture("valid"));
    assert!(diff(&a, &b).is_empty());
}
