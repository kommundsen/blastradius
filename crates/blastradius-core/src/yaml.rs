//! Thin span-aware wrapper over marked-yaml: load a file, hand back the node
//! tree plus helpers that always know their line numbers.

use crate::diagnostics::Diagnostic;
use crate::vfs::Vfs;
use marked_yaml::types::{MarkedMappingNode, MarkedScalarNode, MarkedSequenceNode};
use marked_yaml::Node;

/// Read + parse one YAML file from the workspace source. On failure, pushes a
/// malformed-YAML error (spec §6) and returns None.
pub fn load_file(
    vfs: &dyn Vfs,
    rel: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<(Node, String)> {
    let text = match vfs.read(rel) {
        Ok(t) => t,
        Err(e) => {
            diags.push(Diagnostic::error(rel, 0, format!("cannot read file: {e}")));
            return None;
        }
    };
    parse_str(&text, rel, 0, diags).map(|n| (n, text))
}

/// Parse YAML text with a line offset for diagnostics.
pub fn parse_str(
    text: &str,
    rel: &str,
    line_offset: u64,
    diags: &mut Vec<Diagnostic>,
) -> Option<Node> {
    // marked-yaml's default loader silently keeps the LAST duplicate key,
    // which would make `web:` defined twice collapse into one element with no
    // diagnostic at all. Scan proactively (the Phase 3 carried requirement).
    if let Some(line) = find_duplicate_key_line(text) {
        diags.push(Diagnostic::error(
            rel,
            line + line_offset,
            "malformed YAML: duplicate mapping key",
        ));
        return None;
    }
    match marked_yaml::parse_yaml(0, text) {
        Ok(node) => Some(node),
        Err(e) => {
            // LoadError displays with position info; extract the marker line when
            // present, falling back to a textual scan for duplicate keys.
            let line = load_error_line(&e)
                .or_else(|| {
                    matches!(e, marked_yaml::LoadError::DuplicateKey(..))
                        .then(|| find_duplicate_key_line(text))
                        .flatten()
                })
                .map(|l| l + line_offset)
                .unwrap_or(0);
            diags.push(Diagnostic::error(rel, line, format!("malformed YAML: {e}")));
            None
        }
    }
}

fn load_error_line(e: &marked_yaml::LoadError) -> Option<u64> {
    use marked_yaml::LoadError::*;
    match e {
        TopLevelMustBeMapping(m)
        | UnexpectedAnchor(m)
        | MappingKeyMustBeScalar(m)
        | UnexpectedTag(m) => Some(m.line() as u64),
        ScanError(m, _) => Some(m.line() as u64),
        // marked-yaml's DuplicateKey variant boxes its detail without a Marker;
        // parse_str runs find_duplicate_key_line over the text instead (the
        // Phase 3 carried requirement from spec/sync-engine.md, closed here).
        _ => None,
    }
}

/// Locate the first YAML-level duplicate key textually: track (indent-path,
/// key) pairs per block mapping. Sound for the schema subset (block mappings,
/// one-line flow values) — exactly the documents we accept.
pub(crate) fn find_duplicate_key_line(text: &str) -> Option<u64> {
    use std::collections::HashSet;
    let mut seen: HashSet<(usize, String, String)> = HashSet::new(); // (indent, path, key)
    let mut stack: Vec<(usize, String)> = Vec::new(); // (indent, scope name)
    let mut item_counter: u64 = 0; // globally unique synthetic scope per sequence item
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        let mut t = line.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut indent = line.len() - t.len();
        // A sequence item opens a fresh scope: fields of different items must
        // never collide with each other.
        if let Some(rest) = t.strip_prefix('-') {
            while let Some((top, _)) = stack.last() {
                if *top >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
            item_counter += 1;
            stack.push((indent, format!("[{item_counter}]")));
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }
            // `- key: value` — the key sits inside the item scope, two
            // columns in (dash + space)
            t = rest;
            indent += 2;
        }
        let Some(colon) = t.find(':') else { continue };
        let key = t[..colon].trim().to_string();
        if key.is_empty() || key.contains(' ') {
            continue;
        }
        while let Some((top, _)) = stack.last() {
            if *top >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        let path = stack.iter().map(|(_, k)| k.as_str()).collect::<Vec<_>>().join(".");
        if !seen.insert((indent, path, key.clone())) {
            return Some(i as u64 + 1);
        }
        stack.push((indent, key));
    }
    None
}

/// Line of a node (1-based), best-effort.
pub fn line_of(node: &Node) -> u64 {
    let span = match node {
        Node::Scalar(s) => s.span(),
        Node::Mapping(m) => m.span(),
        Node::Sequence(s) => s.span(),
    };
    span.start().map(|m| m.line() as u64).unwrap_or(0)
}

pub fn scalar_line(s: &MarkedScalarNode) -> u64 {
    s.span().start().map(|m| m.line() as u64).unwrap_or(0)
}

pub fn as_mapping<'a>(
    node: &'a Node,
    rel: &str,
    what: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<&'a MarkedMappingNode> {
    match node {
        Node::Mapping(m) => Some(m),
        other => {
            diags.push(Diagnostic::error(
                rel,
                line_of(other),
                format!("{what} must be a mapping"),
            ));
            None
        }
    }
}

pub fn as_sequence(node: &Node) -> Option<&MarkedSequenceNode> {
    match node {
        Node::Sequence(s) => Some(s),
        _ => None,
    }
}

pub fn as_str(node: &Node) -> Option<&str> {
    match node {
        Node::Scalar(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Get a string field from a mapping.
pub fn get_str<'a>(map: &'a MarkedMappingNode, key: &str) -> Option<&'a str> {
    map.get_node(key).and_then(as_str)
}

/// Get a field's line, falling back to the mapping's own line.
pub fn field_line(map: &MarkedMappingNode, key: &str) -> u64 {
    map.get_node(key)
        .map(line_of)
        .filter(|l| *l > 0)
        .unwrap_or_else(|| map.span().start().map(|m| m.line() as u64).unwrap_or(0))
}
