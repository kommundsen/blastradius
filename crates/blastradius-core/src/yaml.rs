//! Thin span-aware wrapper over marked-yaml: load a file, hand back the node
//! tree plus helpers that always know their line numbers.

use crate::diagnostics::Diagnostic;
use marked_yaml::types::{MarkedMappingNode, MarkedScalarNode, MarkedSequenceNode};
use marked_yaml::Node;
use std::path::Path;

/// Read + parse one YAML file. On failure, pushes a malformed-YAML error
/// (spec §6) and returns None. `line_offset` shifts reported lines — used for
/// frontmatter, which starts mid-file.
pub fn load_file(
    abs: &Path,
    rel: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<(Node, String)> {
    let text = match std::fs::read_to_string(abs) {
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
    match marked_yaml::parse_yaml(0, text) {
        Ok(node) => Some(node),
        Err(e) => {
            // LoadError displays with position info; extract the marker line when present.
            let line = load_error_line(&e).map(|l| l + line_offset).unwrap_or(0);
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
        _ => None,
    }
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
