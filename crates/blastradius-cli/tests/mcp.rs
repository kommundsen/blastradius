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
