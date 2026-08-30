//! MCP server tests (ADR-0012): the JSON-RPC dispatch driven in-process on a
//! scaffolded workspace, plus one real spawned-binary stdio handshake.

use blastradius_cli::mcp::McpServer;
use serde_json::{json, Value};
use std::path::PathBuf;

fn scaffolded(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blastradius-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (rel, text) in blastradius_core::scaffold::starter_workspace("Acme Payments") {
        let path = dir.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    if let Some(j) = blastradius_core::sync::journal_path(&dir) {
        let _ = std::fs::remove_file(j);
    }
    dir
}

fn call(server: &mut McpServer, id: u64, method: &str, params: Value) -> Value {
    let resp = server
        .handle(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
        .expect("requests get responses");
    assert_eq!(resp["id"], json!(id));
    resp
}

/// tools/call, unwrapping the text content back to JSON.
fn tool(server: &mut McpServer, name: &str, args: Value) -> Result<Value, String> {
    let resp = call(server, 99, "tools/call", json!({"name": name, "arguments": args}));
    let result = &resp["result"];
    let text = result["content"][0]["text"].as_str().expect("text content").to_string();
    if result["isError"].as_bool() == Some(true) {
        Err(text)
    } else {
        Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }
}

#[test]
fn initialize_and_list_tools() {
    let dir = scaffolded("handshake");
    let mut s = McpServer::new(&dir);

    let init = call(&mut s, 1, "initialize", json!({"protocolVersion": "2025-06-18"}));
    assert_eq!(init["result"]["serverInfo"]["name"], "blastradius");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    // notifications get no response
    assert!(s.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());

    let tools = call(&mut s, 2, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "workspace_summary", "find_elements", "element", "blast_radius",
        "validate", "model_diff", "doc", "apply_operation", "undo", "redo",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}: {names:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_tools_answer_from_the_model() {
    let dir = scaffolded("reads");
    let mut s = McpServer::new(&dir);

    let summary = tool(&mut s, "workspace_summary", json!({})).unwrap();
    assert_eq!(summary["workspace"], "Acme Payments");
    assert_eq!(summary["errors"], 0);

    let found = tool(&mut s, "find_elements", json!({"query": "database"})).unwrap();
    assert_eq!(found["elements"][0]["id"], "acme-payments.db");

    let el = tool(&mut s, "element", json!({"id": "acme-payments.app"})).unwrap();
    assert_eq!(el["kind"], "container");
    assert!(el["relations"]["outgoing"].as_array().unwrap().len() >= 2, "{el}");

    // unknown ids come back with suggestions, not a bare error
    let err = tool(&mut s, "element", json!({"id": "acme-payments.ap"})).unwrap_err();
    assert!(err.contains("did you mean"), "{err}");

    let doc = tool(&mut s, "doc", json!({"id": "readme"})).unwrap();
    assert!(doc["body"].as_str().unwrap().contains("# Acme Payments architecture"));
    assert!(!doc["body"].as_str().unwrap().contains("---"), "frontmatter stripped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blast_radius_reports_dependents_docs_views() {
    let dir = scaffolded("blast");
    let mut s = McpServer::new(&dir);
    // db is depended on by app (direct), and user->system via context
    let br = tool(&mut s, "blast_radius", json!({"id": "acme-payments.db"})).unwrap();
    let dependents: Vec<&str> = br["dependents"]
        .as_array().unwrap().iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert!(dependents.contains(&"acme-payments.app"), "{br}");
    // the README governs the parent system -> shows as inherited
    assert_eq!(br["docs"][0]["id"], "readme");
    assert_eq!(br["docs"][0]["governs"], "via parent");
    assert_eq!(br["views"][0], "containers");

    // the whole system: user depends on it transitively via the context relation
    let br = tool(&mut s, "blast_radius", json!({"id": "acme-payments"})).unwrap();
    let dependents: Vec<&str> = br["dependents"]
        .as_array().unwrap().iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert!(dependents.contains(&"user"), "{br}");
    assert_eq!(br["docs"][0]["governs"], "directly");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_go_through_the_sync_engine_and_undo() {
    let dir = scaffolded("writes");
    let mut s = McpServer::new(&dir);

    let before = std::fs::read_to_string(dir.join("model/acme-payments.yaml")).unwrap();
    let applied = tool(&mut s, "apply_operation", json!({"op": {
        "op": "set-field", "id": "acme-payments.db",
        "field": "description", "value": "Primary ledger store"
    }})).unwrap();
    assert_eq!(applied["files"][0], "model/acme-payments.yaml");
    let text = std::fs::read_to_string(dir.join("model/acme-payments.yaml")).unwrap();
    assert!(text.contains("description: Primary ledger store"), "{text}");
    assert!(text.contains("# One file per software system"), "comments survive: {text}");

    // invalid ops are refused with the engine's message
    let err = tool(&mut s, "apply_operation", json!({"op": {
        "op": "create", "parent": null, "id": "Bad Id", "name": "x", "kind": "system"
    }})).unwrap_err();
    assert!(err.contains("slug"), "{err}");

    // undo restores byte-identical
    tool(&mut s, "undo", json!({})).unwrap();
    let after = std::fs::read_to_string(dir.join("model/acme-payments.yaml")).unwrap();
    assert_eq!(after, before, "undo across MCP is byte-exact");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validate_sees_external_breakage_and_recovery() {
    let dir = scaffolded("validate");
    let mut s = McpServer::new(&dir);
    let good = std::fs::read_to_string(dir.join("model/context.yaml")).unwrap();
    std::fs::write(dir.join("model/context.yaml"), "people:\n   broken: [indent\n").unwrap();
    let v = tool(&mut s, "validate", json!({})).unwrap();
    assert_eq!(v["result"], "FAIL");
    assert!(v["diagnostics"].as_array().unwrap().iter().any(|d| d["file"] == "model/context.yaml"));
    std::fs::write(dir.join("model/context.yaml"), good).unwrap();
    let v = tool(&mut s, "validate", json!({})).unwrap();
    assert_eq!(v["result"], "PASS");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real transport: spawn the binary, shake hands over stdio.
#[test]
fn spawned_binary_speaks_mcp_over_stdio() {
    use std::io::{BufRead, BufReader, Write};
    let dir = scaffolded("stdio");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .arg("mcp")
        .arg(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("binary spawns");
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18"}}}}"#).unwrap();
    let resp: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(resp["result"]["serverInfo"]["name"], "blastradius");

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"workspace_summary","arguments":{{}}}}}}"#).unwrap();
    let resp: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert!(resp["result"]["content"][0]["text"].as_str().unwrap().contains("Acme Payments"));

    drop(stdin); // EOF ends the serve loop
    let status = child.wait().unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

/// L4 introspection through the server (spec/l4-introspection.md): the
/// introspect tool extracts and commits facts, derived elements answer in the
/// read tools, and apply_operation refuses to touch them.
#[test]
fn introspect_tool_extracts_and_derived_elements_answer() {
    let dir = scaffolded("introspect");
    // Make the workspace dir a repo root with a small Rust source tree.
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/engine.rs"),
        "pub struct Engine;\n\npub struct Plan;\n\nimpl Engine {\n    pub fn plan(&self) -> Plan {\n        Plan\n    }\n}\n",
    )
    .unwrap();
    // Opt a component in by hand (source: is not an op-editable field).
    let model = dir.join("model/acme-payments.yaml");
    let text = std::fs::read_to_string(&model).unwrap();
    let patched = text.replace(
        "\x20 app:\n    name: Application\n",
        "\x20 app:\n    name: Application\n    components:\n      engine:\n        name: Engine\n        source:\n          language: rust\n          root: src\n",
    );
    assert_ne!(patched, text, "scaffold shape changed — fix the patch anchor");
    std::fs::write(&model, patched).unwrap();

    let mut s = McpServer::new(&dir);
    let out = tool(&mut s, "introspect", json!({})).unwrap();
    assert_eq!(out["results"][0]["component"], "acme-payments.app.engine");
    assert_eq!(out["results"][0]["written"], true);
    assert!(dir.join("model/derived/acme-payments.app.engine.l4.json").is_file());

    // Read tools see the derived elements, marked.
    let found = tool(&mut s, "find_elements", json!({"query": "Engine", "kind": "class"})).unwrap();
    let ids: Vec<&str> = found["elements"].as_array().unwrap().iter().filter_map(|e| e["id"].as_str()).collect();
    assert!(ids.contains(&"acme-payments.app.engine.src.engine.Engine"), "{ids:?}");

    let el = tool(&mut s, "element", json!({"id": "acme-payments.app.engine.src.engine.Engine"})).unwrap();
    assert_eq!(el["derived"], true);
    assert_eq!(el["path"], "src/engine.rs");

    let br = tool(&mut s, "blast_radius", json!({"id": "acme-payments.app.engine.src.engine.Plan"})).unwrap();
    let dependents: Vec<&str> = br["dependents"].as_array().unwrap().iter().filter_map(|d| d["id"].as_str()).collect();
    assert!(dependents.contains(&"acme-payments.app.engine.src.engine.Engine"), "{dependents:?}");

    // Derived ids are read-only through the write path.
    let err = tool(
        &mut s,
        "apply_operation",
        json!({"op": {"op": "set-field", "id": "acme-payments.app.engine.src.engine.Engine", "field": "description", "value": "x"}}),
    )
    .unwrap_err();
    assert!(err.contains("derived from source"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// 0.3.0 theme 2 exit criterion (ADR-0015 follow-up): an MCP client resolves
/// a manufactured merge conflict end-to-end through the server — sees it via
/// git_conflicts, decides per element, resolve_conflicts splices + stages,
/// and the workspace comes back clean.
#[test]
fn mcp_client_resolves_a_merge_conflict_end_to_end() {
    use git2::{Repository, Signature};

    let dir = std::env::temp_dir().join(format!("blastradius-mcp-resolve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // comment on purpose: the resolution must carry it through untouched
    const SHOP: &str = "system: shop\ncontainers:\n  # storefront — reviewed quarterly\n  web:\n    name: Web App\n    tech: React\n";
    // Scope every git2 object: the manufacture borrows must all die before
    // the cleanup at the end of the test.
    {
    let repo = Repository::init(&dir).unwrap();
    let sig = Signature::now("test", "test@example.com").unwrap();
    let write = |rel: &str, text: &str| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    let commit_all = |msg: &str| {
        let mut index = repo.index().unwrap();
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap()
    };

    write("blastradius.yaml", "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\n");
    write("model/shop.yaml", SHOP);
    let base = commit_all("base");

    write("model/shop.yaml", &SHOP.replace("name: Web App", "name: Storefront"));
    let mine = commit_all("mine");

    let base_commit = repo.find_commit(base).unwrap();
    repo.branch("side", &base_commit, false).unwrap();
    repo.set_head("refs/heads/side").unwrap();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
    write("model/shop.yaml", &SHOP.replace("name: Web App", "name: Shop Frontend"));
    commit_all("side");
    let mine_ac = repo.find_annotated_commit(mine).unwrap();
    repo.merge(&[&mine_ac], None, None).unwrap();
    assert!(repo.index().unwrap().has_conflicts(), "merge must conflict");
    }

    // The agent's view: status shows the conflict, git_conflicts shapes it.
    let mut s = McpServer::new(&dir);
    let status = tool(&mut s, "git_status", json!({})).unwrap();
    assert_eq!(status["repository"], true);
    let conflicts = tool(&mut s, "git_conflicts", json!({})).unwrap();
    let el = &conflicts["conflicts"]["elements"][0];
    assert_eq!(el["id"], "shop.web");
    assert_eq!(el["ours"]["name"], "Shop Frontend");
    assert_eq!(el["theirs"]["name"], "Storefront");

    // Decide: take the incoming rename.
    let out = tool(
        &mut s,
        "resolve_conflicts",
        json!({"resolution": {"elements": {"shop.web": "theirs"}}}),
    )
    .unwrap();
    assert_eq!(out["staged"][0], "model/shop.yaml");

    // Byte-clean on the ours base (comment intact), staged, conflict gone.
    let text = std::fs::read_to_string(dir.join("model/shop.yaml")).unwrap();
    assert_eq!(text, SHOP.replace("name: Web App", "name: Storefront"));
    assert!(text.contains("# storefront — reviewed quarterly"));
    let reopened = Repository::open(&dir).unwrap();
    assert!(!reopened.index().unwrap().has_conflicts(), "resolution must be staged");
    drop(reopened);
    let after = tool(&mut s, "git_conflicts", json!({})).unwrap();
    assert_eq!(after["conflicts"], Value::Null);
    // and the reloaded model serves the chosen name
    let elx = tool(&mut s, "element", json!({"id": "shop.web"})).unwrap();
    assert_eq!(elx["name"], "Storefront");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the agent-facing surface (docs/roadmap.md, first-user findings) -------
//
// The first agent to model a repository with these tools read the model
// through them, then wrote YAML by hand and looped on validation errors. It
// had nothing to write against: no tool returned the format, apply_operation's
// schema was `{"op": {"type": "object"}}`, and building a model from scratch
// meant dozens of single calls. These three tests cover the three gaps.

#[test]
fn model_format_is_reachable_and_says_what_matters() {
    let dir = scaffolded("format");
    let mut s = McpServer::new(&dir);

    let names: Vec<String> = call(&mut s, 1, "tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for expected in ["model_format", "apply_operations"] {
        assert!(names.iter().any(|n| n == expected), "missing tool {expected}: {names:?}");
    }

    let text = tool(&mut s, "model_format", json!({})).unwrap();
    let text = text["format"].as_str().expect("format is a string");
    for expected in [
        "blastradius.yaml", // the manifest
        "immutable",        // ids never change
        "dependency, not a data flow",
        "container-instance",
        "show-groups",
        "doc: adr-0001", // the worked example is in there
    ] {
        assert!(text.contains(expected), "model_format never mentions {expected:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_operation_publishes_the_shapes_it_accepts() {
    let dir = scaffolded("schema");
    let mut s = McpServer::new(&dir);
    let tools = call(&mut s, 1, "tools/list", json!({}))["result"]["tools"].clone();
    let apply = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "apply_operation")
        .expect("apply_operation");

    // Every variant of sync::Operation must be a branch a caller can see.
    let branches = apply["inputSchema"]["properties"]["op"]["oneOf"].as_array().expect("oneOf");
    let ops: Vec<&str> = branches.iter().map(|b| b["properties"]["op"]["const"].as_str().unwrap()).collect();
    for expected in [
        "create", "rename", "set-field", "delete",
        "add-relation", "delete-relation", "set-relation-field", "reverse-relation",
        "pin", "unpin", "show-description", "set-source", "set-view-flag",
    ] {
        assert!(ops.contains(&expected), "operation {expected} is not in the schema: {ops:?}");
    }
    assert_eq!(ops.len(), 13, "a variant was added to Operation without a schema branch: {ops:?}");

    // And a malformed call comes back naming the shape, not just serde's
    // "missing field".
    let err = tool(&mut s, "apply_operation", json!({"op": {"op": "create", "id": "x"}})).unwrap_err();
    assert!(err.contains("create"), "{err}");
}

#[test]
fn apply_operations_builds_a_model_in_one_transaction() {
    let dir = scaffolded("batch");
    let mut s = McpServer::new(&dir);

    let before = tool(&mut s, "workspace_summary", json!({})).unwrap();
    let ops = json!([
        {"op": "create", "parent": "acme-payments", "id": "ledger", "name": "Ledger", "kind": "container"},
        {"op": "create", "parent": "acme-payments.ledger", "id": "posting", "name": "Posting", "kind": "component"},
        {"op": "set-field", "id": "acme-payments.ledger", "field": "tech", "value": "Rust"},
        {"op": "add-relation", "from": "acme-payments.app", "to": "acme-payments.ledger",
         "label": "posts to", "protocol": "gRPC"},
    ]);
    let res = tool(&mut s, "apply_operations", json!({"ops": ops})).unwrap();
    assert_eq!(res["operations"], json!(4));

    let el = tool(&mut s, "element", json!({"id": "acme-payments.ledger"})).unwrap();
    assert_eq!(el["tech"], "Rust");
    assert!(tool(&mut s, "element", json!({"id": "acme-payments.ledger.posting"})).is_ok());

    // One undo takes the whole batch back — not four.
    tool(&mut s, "undo", json!({})).unwrap();
    assert!(tool(&mut s, "element", json!({"id": "acme-payments.ledger"})).is_err());
    let after = tool(&mut s, "workspace_summary", json!({})).unwrap();
    assert_eq!(after["counts"], before["counts"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_operation_rolls_the_whole_batch_back() {
    let dir = scaffolded("rollback");
    let mut s = McpServer::new(&dir);
    let before = tool(&mut s, "workspace_summary", json!({})).unwrap();

    let err = tool(
        &mut s,
        "apply_operations",
        json!({"ops": [
            {"op": "create", "parent": "acme-payments", "id": "ledger", "name": "Ledger", "kind": "container"},
            // Dangling target: the workspace would not validate.
            {"op": "add-relation", "from": "acme-payments.ledger", "to": "acme-payments.nowhere"},
        ]}),
    )
    .unwrap_err();
    assert!(err.contains("operation 2"), "{err}");
    assert!(err.contains("rolled back"), "{err}");

    assert!(tool(&mut s, "element", json!({"id": "acme-payments.ledger"})).is_err());
    let after = tool(&mut s, "workspace_summary", json!({})).unwrap();
    assert_eq!(after["counts"], before["counts"]);
    assert_eq!(after["errors"], json!(0));
    let _ = std::fs::remove_dir_all(&dir);
}
