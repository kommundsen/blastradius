//! MCP server (ADR-0012): a third head on the core library, next to the CLI
//! commands and the Tauri shell (ADR-0005). Speaks the Model Context Protocol
//! over stdio so coding agents can query the model with task-shaped tools —
//! and edit it through the sync engine, whose CST-preserving splices keep
//! agent edits indistinguishable from hand edits in the diff.
//!
//! Hand-rolled on purpose: MCP's stdio transport is newline-delimited
//! JSON-RPC 2.0 and this server needs three methods (initialize, tools/list,
//! tools/call). An SDK would bring an async runtime for a protocol we can
//! serve with a read-line loop — same reasoning as the vendored-libgit2 and
//! hand-rolled Structurizr choices.

use blastradius_core::diagnostics::Severity;
use blastradius_core::git::GitContext;
use blastradius_core::model::Workspace;
use blastradius_core::sync::{Operation, SyncEngine};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub struct McpServer {
    root: PathBuf,
    engine: SyncEngine,
}

/// Resolve the workspace folder like the desktop app does: the argument, or
/// the current directory, falling back to `./docs` (the dogfood layout).
pub fn resolve_root(arg: Option<&str>) -> Result<PathBuf, String> {
    let base = PathBuf::from(arg.unwrap_or("."));
    let candidates = [base.clone(), base.join("docs")];
    for c in &candidates {
        if c.join("workspace.yaml").is_file() {
            return Ok(c.canonicalize().unwrap_or_else(|_| c.clone()));
        }
    }
    Err(format!(
        "{}: no workspace.yaml here or in ./docs — pass a workspace folder (or run `blastradius init`)",
        base.display()
    ))
}

/// Blocking serve loop: one JSON-RPC message per stdin line, responses on
/// stdout, logs on stderr (the MCP stdio contract).
pub fn serve(arg: Option<&str>) -> Result<(), String> {
    let root = resolve_root(arg)?;
    let mut server = McpServer::new(&root);
    eprintln!("blastradius mcp: serving {}", root.display());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({"jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")}});
                writeln!(stdout, "{err}").map_err(|e| e.to_string())?;
                stdout.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };
        if let Some(resp) = server.handle(&msg) {
            writeln!(stdout, "{resp}").map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

impl McpServer {
    pub fn new(root: &Path) -> Self {
        McpServer { root: root.to_path_buf(), engine: SyncEngine::open(root) }
    }

    /// Dispatch one JSON-RPC message; None for notifications (no response).
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let method = msg.get("method")?.as_str()?;
        let id = msg.get("id").cloned();
        // notifications carry no id and get no response
        if id.is_none() || id == Some(Value::Null) {
            return None;
        }
        let id = id.unwrap();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion")
                    .and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "blastradius",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "Query and edit the Blastradius architecture model \
                    (C4, YAML-in-repo). Start with workspace_summary; use blast_radius \
                    before changing an element; edits via apply_operation are \
                    format-preserving splices, and undo reverts the last one.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let outcome = self.call_tool(name, &args);
                let (text, is_error) = match outcome {
                    Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                    Err(e) => (e, true),
                };
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": is_error,
                }))
            }
            other => Err(format!("method not found: {other}")),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(m) => json!({"jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": m}}),
        })
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        // agents also edit files directly — pick up external changes first,
        // exactly like the app's watcher path (echo suppression included)
        self.engine.external_scan();
        let str_arg = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        match name {
            "workspace_summary" => Ok(self.workspace_summary()),
            "find_elements" => Ok(self.find_elements(str_arg("query"), str_arg("kind"))),
            "element" => self.element(&str_arg("id").ok_or("id is required")?),
            "blast_radius" => self.blast_radius(&str_arg("id").ok_or("id is required")?),
            "validate" => Ok(self.validate()),
            "model_diff" => self.model_diff(str_arg("base")),
            "doc" => self.doc(&str_arg("id").ok_or("id is required")?),
            "apply_operation" => {
                let op = args.get("op").cloned().ok_or("op is required")?;
                let op: Operation =
                    serde_json::from_value(op).map_err(|e| format!("bad operation: {e}"))?;
                let tx = self.engine.apply(op)?;
                Ok(json!({
                    "applied": tx.label,
                    "files": tx.changes.iter().map(|c| c.rel.clone()).collect::<Vec<_>>(),
                    "diagnostics": self.diagnostics_json(),
                }))
            }
            "undo" => {
                let label = self.engine.undo()?;
                Ok(json!({ "undone": label }))
            }
            "redo" => {
                let label = self.engine.redo()?;
                Ok(json!({ "redone": label }))
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn ws(&self) -> &Workspace {
        &self.engine.model
    }

    fn diagnostics_json(&self) -> Vec<Value> {
        self.engine
            .diagnostics
            .iter()
            .map(|d| {
                json!({
                    "severity": format!("{:?}", d.severity).to_lowercase(),
                    "file": d.file, "line": d.line, "message": d.message,
                })
            })
            .collect()
    }

    fn element_brief(&self, id: &str) -> Value {
        let el = &self.ws().elements[id];
        let mut v = json!({
            "id": el.id, "kind": el.kind.as_str(), "name": el.name,
            "file": el.file, "line": el.line,
        });
        if let Some(t) = &el.tech {
            v["tech"] = json!(t);
        }
        if let Some(d) = &el.description {
            v["description"] = json!(d);
        }
        v
    }

    fn workspace_summary(&mut self) -> Value {
        let ws = self.ws();
        let mut by_kind = std::collections::BTreeMap::new();
        for el in ws.elements.values() {
            *by_kind.entry(el.kind.as_str()).or_insert(0u64) += 1;
        }
        let systems: Vec<Value> = ws
            .elements
            .values()
            .filter(|e| e.kind == blastradius_core::model::ElementKind::System)
            .map(|e| {
                let children = ws.elements.values().filter(|c| {
                    c.id.strip_prefix(&format!("{}.", e.id))
                        .is_some_and(|rest| !rest.contains('.'))
                });
                json!({
                    "id": e.id, "name": e.name,
                    "containers": children.map(|c| c.id.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({
            "workspace": ws.name,
            "root": self.root.display().to_string(),
            "elements": by_kind,
            "systems": systems,
            "views": ws.views.iter().map(|v| json!({
                "id": v.id, "level": v.level, "scope": v.scope, "pins": v.layout.len(),
            })).collect::<Vec<_>>(),
            "docs": ws.docs.iter().map(|d| json!({
                "id": d.id, "type": d.doc_type, "status": d.status, "file": d.file,
            })).collect::<Vec<_>>(),
            "stale_files": self.engine.stale.iter().collect::<Vec<_>>(),
            "errors": self.engine.diagnostics.iter()
                .filter(|d| d.severity == Severity::Error).count(),
        })
    }

    fn find_elements(&self, query: Option<String>, kind: Option<String>) -> Value {
        let q = query.unwrap_or_default().to_lowercase();
        let matches: Vec<&str> = self
            .ws()
            .elements
            .values()
            .filter(|e| {
                let kind_ok = kind.as_deref().is_none_or(|k| {
                    e.kind.as_str().starts_with(k) || (k == "external" && e.kind.as_str().contains("external"))
                });
                let text_ok = q.is_empty()
                    || e.id.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.description.as_deref().unwrap_or("").to_lowercase().contains(&q);
                kind_ok && text_ok
            })
            .map(|e| e.id.as_str())
            .collect();
        let capped: Vec<Value> = matches.iter().take(50).map(|id| self.element_brief(id)).collect();
        json!({
            "total": matches.len(),
            "shown": capped.len(),
            "elements": capped,
        })
    }

    fn element(&self, id: &str) -> Result<Value, String> {
        let ws = self.ws();
        let el = ws.elements.get(id).ok_or_else(|| unknown_element(ws, id))?;
        let child_prefix = format!("{id}.");
        let children: Vec<&str> = ws
            .elements
            .keys()
            .filter(|k| k.strip_prefix(&child_prefix).is_some_and(|r| !r.contains('.')))
            .map(String::as_str)
            .collect();
        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();
        for (from, to, r) in ws.resolved_relations() {
            let rel = |peer: &str| {
                json!({
                    "peer": peer, "label": r.label, "protocol": r.protocol,
                    "file": r.file, "line": r.line,
                })
            };
            if from == id || from.starts_with(&child_prefix) {
                outgoing.push(rel(&to));
            }
            if to == id || to.starts_with(&child_prefix) {
                incoming.push(rel(&from));
            }
        }
        let docs: Vec<Value> = ws
            .docs
            .iter()
            .filter(|d| d.elements.iter().any(|e| e == id))
            .map(|d| json!({"id": d.id, "type": d.doc_type, "file": d.file}))
            .collect();
        let views: Vec<&str> = ws
            .views
            .iter()
            .filter(|v| {
                v.scope == id
                    || v.scope.starts_with(&child_prefix)
                    || id.starts_with(&format!("{}.", v.scope))
            })
            .map(|v| v.id.as_str())
            .collect();
        let mut out = self.element_brief(&el.id);
        out["external"] = json!(el.external);
        out["children"] = json!(children);
        out["relations"] = json!({"outgoing": outgoing, "incoming": incoming});
        out["docs"] = json!(docs);
        out["views"] = json!(views);
        Ok(out)
    }

    /// The tool the product is named for: everything implicated when this
    /// element changes — contents, transitive dependents (reverse reachability
    /// over relations), direct dependencies, governing docs, affected views.
    fn blast_radius(&self, id: &str) -> Result<Value, String> {
        let ws = self.ws();
        if !ws.elements.contains_key(id) {
            return Err(unknown_element(ws, id));
        }
        let child_prefix = format!("{id}.");
        let in_target = |e: &str| e == id || e.starts_with(&child_prefix);

        let resolved = ws.resolved_relations();
        // reverse BFS: who (transitively) depends on the target subtree
        let mut dist: std::collections::BTreeMap<String, u64> = Default::default();
        let mut frontier: Vec<String> = vec![id.to_string()];
        let mut depth = 0u64;
        while !frontier.is_empty() {
            depth += 1;
            let mut next = Vec::new();
            for (from, to, _) in &resolved {
                let hits = if depth == 1 {
                    in_target(to)
                } else {
                    frontier.iter().any(|f| to == f || to.starts_with(&format!("{f}.")))
                };
                if hits && !in_target(from) && !dist.contains_key(from) {
                    dist.insert(from.clone(), depth);
                    next.push(from.clone());
                }
            }
            frontier = next;
        }
        let dependencies: Vec<Value> = resolved
            .iter()
            .filter(|(from, to, _)| in_target(from) && !in_target(to))
            .map(|(_, to, r)| json!({"id": to, "label": r.label}))
            .collect();
        let ancestors: Vec<&str> = {
            let mut v = Vec::new();
            let mut cur = id;
            while let Some(pos) = cur.rfind('.') {
                cur = &cur[..pos];
                v.push(cur);
            }
            v
        };
        let docs: Vec<Value> = ws
            .docs
            .iter()
            .filter_map(|d| {
                let direct = d.elements.iter().any(|e| in_target(e));
                let inherited = !direct && d.elements.iter().any(|e| ancestors.contains(&e.as_str()));
                (direct || inherited).then(|| {
                    json!({"id": d.id, "type": d.doc_type, "file": d.file,
                        "governs": if direct { "directly" } else { "via parent" }})
                })
            })
            .collect();
        let views: Vec<&str> = ws
            .views
            .iter()
            .filter(|v| {
                in_target(&v.scope)
                    || id.starts_with(&format!("{}.", v.scope))
                    || ancestors.contains(&v.scope.as_str())
            })
            .map(|v| v.id.as_str())
            .collect();
        let mut dependents: Vec<Value> = dist
            .iter()
            .map(|(e, d)| json!({"id": e, "distance": d}))
            .collect();
        dependents.sort_by_key(|v| v["distance"].as_u64());
        Ok(json!({
            "target": id,
            "contains": ws.elements.keys()
                .filter(|k| k.starts_with(&child_prefix)).collect::<Vec<_>>(),
            "dependents": dependents,
            "dependencies": dependencies,
            "docs": docs,
            "views": views,
        }))
    }

    fn validate(&mut self) -> Value {
        // a full fresh parse — identical to `blastradius validate`
        self.engine.reload_all();
        let errors =
            self.engine.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
        json!({
            "result": if errors == 0 { "PASS" } else { "FAIL" },
            "elements": self.ws().elements.len(),
            "diagnostics": self.diagnostics_json(),
        })
    }

    fn model_diff(&self, base: Option<String>) -> Result<Value, String> {
        let ctx = GitContext::discover(&self.root)
            .ok_or("workspace is not inside a git repository")?;
        let base_label = base
            .or_else(|| ctx.default_base())
            .ok_or("no base ref: pass base explicitly (e.g. \"HEAD\" or a branch)")?;
        let (base_ws, base_diags) = ctx.load_at(&base_label)?;
        if blastradius_core::diagnostics::has_errors(&base_diags) {
            return Err(format!("base revision {base_label} does not parse"));
        }
        let (cur_ws, cur_diags) = blastradius_core::load_workspace(&self.root);
        if blastradius_core::diagnostics::has_errors(&cur_diags) {
            return Err("working tree does not parse".to_string());
        }
        let payload = blastradius_core::diff::diff_payload(&base_label, &base_ws, &cur_ws);
        serde_json::to_value(&payload).map_err(|e| e.to_string())
    }

    fn doc(&self, id: &str) -> Result<Value, String> {
        let ws = self.ws();
        let d = ws.docs.iter().find(|d| d.id == id).ok_or_else(|| {
            format!(
                "unknown doc {id:?} — known: {}",
                ws.docs.iter().map(|d| d.id.as_str()).collect::<Vec<_>>().join(", ")
            )
        })?;
        let raw = std::fs::read_to_string(self.root.join(&d.file)).map_err(|e| e.to_string())?;
        // strip the frontmatter block; the metadata rides along as fields
        let body = match raw.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")) {
            Some((_, b)) => b,
            None => raw.as_str(),
        };
        Ok(json!({
            "id": d.id, "type": d.doc_type, "status": d.status,
            "elements": d.elements, "file": d.file, "body": body,
        }))
    }
}

fn unknown_element(ws: &Workspace, id: &str) -> String {
    let near: Vec<&str> = ws
        .elements
        .keys()
        .filter(|k| k.contains(id.rsplit('.').next().unwrap_or(id)))
        .take(5)
        .map(String::as_str)
        .collect();
    if near.is_empty() {
        format!("unknown element {id:?} — try find_elements")
    } else {
        format!("unknown element {id:?} — did you mean: {}", near.join(", "))
    }
}

fn tool_definitions() -> Vec<Value> {
    let obj = |props: Value, required: Vec<&str>| {
        json!({"type": "object", "properties": props, "required": required})
    };
    vec![
        json!({
            "name": "workspace_summary",
            "description": "Orientation: workspace name, element counts by kind, systems with their containers, views, docs, and current validation state. Call this first.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "find_elements",
            "description": "Search elements by substring (id, name, description) and/or kind. Returns up to 50 briefs with file:line locations.",
            "inputSchema": obj(json!({
                "query": {"type": "string", "description": "case-insensitive substring"},
                "kind": {"type": "string", "enum": ["person", "external", "system", "container", "component"]},
            }), vec![]),
        }),
        json!({
            "name": "element",
            "description": "Full detail for one element by dotted id: fields, children, resolved incoming/outgoing relations, governing docs, views it appears in.",
            "inputSchema": obj(json!({"id": {"type": "string", "description": "dotted element id, e.g. shop.api"}}), vec!["id"]),
        }),
        json!({
            "name": "blast_radius",
            "description": "Impact analysis: everything implicated if this element changes — its contents, transitive dependents (with distance), direct dependencies, governing docs (direct and via parent), and affected views. Use before modifying or deleting anything.",
            "inputSchema": obj(json!({"id": {"type": "string"}}), vec!["id"]),
        }),
        json!({
            "name": "validate",
            "description": "Re-parse and validate the workspace from disk. Returns PASS/FAIL plus every diagnostic with file and line. Run after editing YAML directly.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "model_diff",
            "description": "Semantic model diff of the working tree against a git ref (default: merge-base with the default branch): elements and relations added/removed/changed, layout-only changes separated.",
            "inputSchema": obj(json!({"base": {"type": "string", "description": "git ref; defaults to the merge-base"}}), vec![]),
        }),
        json!({
            "name": "doc",
            "description": "Read a model-registered document (ADR, spec, PRD...) by doc id: metadata, linked elements, and the markdown body.",
            "inputSchema": obj(json!({"id": {"type": "string", "description": "doc id from workspace_summary"}}), vec!["id"]),
        }),
        json!({
            "name": "apply_operation",
            "description": "Edit the model through the sync engine: a format-preserving targeted splice (comments, key order, formatting survive — never re-serialize YAML yourself). Validated before writing; refused if it would invalidate the workspace. Ops: {op: 'create', parent?, id, name, kind} | {op: 'rename', id, name} | {op: 'set-field', id, field: name|description|tech, value} | {op: 'delete', id} | {op: 'add-relation', from, to, label?, protocol?} | {op: 'delete-relation', from, to, label?} | {op: 'set-relation-field', from, to, label?, field: label|protocol, value} | {op: 'pin', view?, level, scope?, id, x, y}.",
            "inputSchema": obj(json!({"op": {"type": "object", "description": "the operation object (see tool description)"}}), vec!["op"]),
        }),
        json!({
            "name": "undo",
            "description": "Undo the last transaction (shared history: canvas, MCP, and external edits alike).",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "redo",
            "description": "Redo the last undone transaction.",
            "inputSchema": obj(json!({}), vec![]),
        }),
    ]
}
