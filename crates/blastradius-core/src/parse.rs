//! Model-file parsing (spec §3): context files (people/external) and system
//! files. Produces elements + raw relations; reference *resolution* stays in
//! `validate` so that every dangling id is reported, not just the first.

use crate::diagnostics::Diagnostic;
use crate::model::*;
use crate::vfs::Vfs;
use crate::yaml;
use marked_yaml::types::MarkedMappingNode;
use marked_yaml::Node;

/// Raw relation before resolution — validate::cross_validate resolves.
#[derive(Debug, Clone)]
pub struct RawRelation {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub protocol: Option<String>,
    pub direction: Direction,
    /// System file scope for sibling resolution (None never occurs today —
    /// relations only appear in system files).
    pub system: Option<String>,
    pub file: String,
    pub line: u64,
}

pub fn parse_model_file(
    vfs: &dyn Vfs,
    rel: &str,
    ws: &mut Workspace,
    diags: &mut Vec<Diagnostic>,
) {
    let Some((node, _)) = yaml::load_file(vfs, rel, diags) else {
        return;
    };
    let Some(map) = yaml::as_mapping(&node, rel, "model file", diags) else {
        return;
    };

    let has_context = map.get_node("people").is_some() || map.get_node("external").is_some();
    let has_system = map.get_node("system").is_some();
    match (has_context, has_system) {
        (true, true) => {
            diags.push(Diagnostic::error(
                rel,
                1,
                "a model file declares either context elements or one system, not both (spec §3)",
            ));
            return;
        }
        (false, false) => {
            diags.push(Diagnostic::error(
                rel,
                1,
                "model file declares neither context (people/external) nor a system",
            ));
            return;
        }
        _ => {}
    }

    if has_context {
        parse_context_section(map, "people", ElementKind::Person, rel, ws, diags);
        parse_context_section(map, "external", ElementKind::External, rel, ws, diags);
    } else {
        parse_system(map, rel, ws, diags);
    }
}

fn register(ws: &mut Workspace, el: Element, diags: &mut Vec<Diagnostic>) {
    if let Some(existing) = ws.elements.get(&el.id) {
        diags.push(Diagnostic::error(
            &el.file as &str,
            el.line,
            format!("duplicate id {:?} (already declared in {}:{})", el.id, existing.file, existing.line),
        ));
        return;
    }
    ws.elements.insert(el.id.clone(), el);
}

fn parse_context_section(
    map: &MarkedMappingNode,
    section: &str,
    kind: ElementKind,
    rel: &str,
    ws: &mut Workspace,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(Node::Mapping(sec)) = map.get_node(section) else {
        return;
    };
    for (key, body) in sec.iter() {
        let id = key.as_str().to_string();
        let line = yaml::scalar_line(key);
        if !is_valid_slug(&id) {
            diags.push(Diagnostic::error(rel, line, format!("bad id {id:?} — ids are lowercase slugs (ADR-0003)")));
            continue;
        }
        let (name, tech, description) = fields(body, &id);
        register(
            ws,
            Element { id, kind, name, tech, description, external: kind == ElementKind::External, file: rel.to_string(), line },
            diags,
        );
    }
}

fn parse_system(map: &MarkedMappingNode, rel: &str, ws: &mut Workspace, diags: &mut Vec<Diagnostic>) {
    let Some(sid) = yaml::get_str(map, "system").map(str::to_string) else {
        diags.push(Diagnostic::error(rel, yaml::field_line(map, "system"), "`system:` must be a scalar id"));
        return;
    };
    let sys_line = yaml::field_line(map, "system");
    if !is_valid_slug(&sid) {
        diags.push(Diagnostic::error(rel, sys_line, format!("bad system id {sid:?}")));
        return;
    }
    let external = yaml::get_str(map, "external") == Some("true");
    register(
        ws,
        Element {
            id: sid.clone(),
            kind: ElementKind::System,
            name: yaml::get_str(map, "name").map(str::to_string).unwrap_or_else(|| titleize(&sid)),
            tech: yaml::get_str(map, "tech").map(str::to_string),
            description: yaml::get_str(map, "description").map(str::to_string),
            external,
            file: rel.to_string(),
            line: sys_line,
        },
        diags,
    );

    let mut container_count = 0usize;
    if let Some(Node::Mapping(containers)) = map.get_node("containers") {
        for (ckey, cbody) in containers.iter() {
            let cid = ckey.as_str().to_string();
            let cline = yaml::scalar_line(ckey);
            if !is_valid_slug(&cid) {
                diags.push(Diagnostic::error(rel, cline, format!("bad id {cid:?}")));
                continue;
            }
            container_count += 1;
            let full = format!("{sid}.{cid}");
            let (name, tech, description) = fields(cbody, &cid);
            register(
                ws,
                Element { id: full.clone(), kind: ElementKind::Container, name, tech, description, external: false, file: rel.to_string(), line: cline },
                diags,
            );

            if let Node::Mapping(cmap) = cbody {
                // nested components (L3)
                if let Some(Node::Mapping(comps)) = cmap.get_node("components") {
                    for (kkey, kbody) in comps.iter() {
                        let kid = kkey.as_str().to_string();
                        let kline = yaml::scalar_line(kkey);
                        if !is_valid_slug(&kid) {
                            diags.push(Diagnostic::error(rel, kline, format!("bad id {kid:?}")));
                            continue;
                        }
                        let (name, tech, description) = fields(kbody, &kid);
                        register(
                            ws,
                            Element { id: format!("{full}.{kid}"), kind: ElementKind::Component, name, tech, description, external: false, file: rel.to_string(), line: kline },
                            diags,
                        );
                    }
                }
                // container-scoped relations: `from` defaults to the container (spec §3)
                if let Some(rels) = cmap.get_node("relations") {
                    parse_relations(rels, rel, &sid, Some(&cid), ws, diags);
                }
            }
        }
    }
    if container_count == 0 {
        diags.push(Diagnostic::warning(rel, sys_line, format!("system {sid:?} has no containers")));
    }

    if let Some(rels) = map.get_node("relations") {
        parse_relations(rels, rel, &sid, None, ws, diags);
    }
}

fn parse_relations(
    node: &Node,
    rel: &str,
    system: &str,
    default_from: Option<&str>,
    ws: &mut Workspace,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(seq) = yaml::as_sequence(node) else {
        diags.push(Diagnostic::error(rel, yaml::line_of(node), "`relations:` must be a list"));
        return;
    };
    for item in seq.iter() {
        let line = yaml::line_of(item);
        let Node::Mapping(m) = item else {
            diags.push(Diagnostic::error(rel, line, "relation must be a mapping"));
            continue;
        };
        let from = yaml::get_str(m, "from")
            .map(str::to_string)
            .or_else(|| default_from.map(str::to_string));
        let to = yaml::get_str(m, "to").map(str::to_string);
        let (Some(from), Some(to)) = (from, to) else {
            diags.push(Diagnostic::error(rel, line, "relation needs `from:` and `to:`"));
            continue;
        };
        let direction = match yaml::get_str(m, "direction") {
            None => Direction::Forward,
            Some("both") => Direction::Both,
            Some("none") => Direction::None,
            Some(other) => {
                diags.push(Diagnostic::error(
                    rel,
                    yaml::field_line(m, "direction"),
                    format!("bad direction {other:?} — expected `both` or `none` (forward is the default)"),
                ));
                Direction::Forward
            }
        };
        // Stored raw; resolution + duplicate detection in validate.
        ws.relations.push(Relation {
            from,
            to,
            label: yaml::get_str(m, "label").map(str::to_string),
            protocol: yaml::get_str(m, "protocol").map(str::to_string),
            direction,
            file: rel.to_string(),
            line,
            scope: Some(system.to_string()),
        });
    }
}

fn fields(body: &Node, id: &str) -> (String, Option<String>, Option<String>) {
    match body {
        Node::Mapping(m) => (
            yaml::get_str(m, "name").map(str::to_string).unwrap_or_else(|| titleize(id)),
            yaml::get_str(m, "tech").map(str::to_string),
            yaml::get_str(m, "description").map(str::to_string),
        ),
        _ => (titleize(id), None, None),
    }
}

pub fn titleize(id: &str) -> String {
    id.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
