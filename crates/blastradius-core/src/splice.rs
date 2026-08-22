//! Format-preserving YAML splicing (ADR-0008): every edit is a minimal text
//! change located via marked-yaml markers — comments, key order, blank lines
//! and quoting of untouched content survive by construction, because we never
//! re-serialize a document.
//!
//! Scope: the Blastradius schema subset (block mappings, flow mappings on one
//! line, block sequences of mappings). Not a general YAML editor.

use marked_yaml::types::MarkedMappingNode;
use marked_yaml::Node;

/// A located mapping entry: `key:` line plus the line range of its value.
#[derive(Debug, Clone, Copy)]
pub struct EntrySpan {
    /// 1-based line of the key.
    pub key_line: usize,
    /// 0-based indent column of the key.
    pub indent: usize,
    /// 1-based last line of the entry's value (>= key_line).
    pub end_line: usize,
    /// Value starts on the key line itself (scalar or flow mapping).
    pub inline: bool,
}

fn parse(text: &str) -> Result<Node, String> {
    marked_yaml::parse_yaml(0, text).map_err(|e| e.to_string())
}

fn as_map(node: &Node) -> Option<&MarkedMappingNode> {
    match node {
        Node::Mapping(m) => Some(m),
        _ => None,
    }
}

/// Deepest line of a node's subtree, from START markers only — marked-yaml
/// end markers are exclusive and can point at the token *after* the node
/// (e.g. the next sibling key), which would make spans overshoot. In this
/// schema subset every value ends on the line it starts, so start markers of
/// the deepest descendants are the truth.
fn node_end_line(node: &Node) -> usize {
    fn walk(n: &Node, max: &mut usize) {
        let span = match n {
            Node::Scalar(s) => s.span(),
            Node::Mapping(m) => m.span(),
            Node::Sequence(s) => s.span(),
        };
        if let Some(m) = span.start() {
            *max = (*max).max(m.line());
        }
        match n {
            Node::Mapping(m) => {
                for (k, v) in m.iter() {
                    if let Some(s) = k.span().start() {
                        *max = (*max).max(s.line());
                    }
                    walk(v, max);
                }
            }
            Node::Sequence(s) => {
                for v in s.iter() {
                    walk(v, max);
                }
            }
            Node::Scalar(_) => {}
        }
    }
    let mut max = 0;
    walk(node, &mut max);
    max.max(1)
}

/// Walk a key chain from the document root; return the entry span of the last
/// key. Chain keys address block mappings only (the schema's spine).
pub fn find_entry(text: &str, chain: &[&str]) -> Result<Option<EntrySpan>, String> {
    let root = parse(text)?;
    let mut current = root;
    let mut result: Option<EntrySpan> = None;
    for (i, key) in chain.iter().enumerate() {
        let map = match as_map(&current) {
            Some(m) => m,
            None => return Ok(None),
        };
        let mut found = None;
        for (k, v) in map.iter() {
            if k.as_str() == *key {
                let key_marker = k.span().start().ok_or("key without marker")?;
                let key_line = key_marker.line();
                let indent = key_marker.column().saturating_sub(1);
                let value_start = match v {
                    Node::Scalar(s) => s.span().start().map(|m| m.line()).unwrap_or(key_line),
                    Node::Mapping(m) => m.span().start().map(|m| m.line()).unwrap_or(key_line),
                    Node::Sequence(s) => s.span().start().map(|m| m.line()).unwrap_or(key_line),
                };
                let end_line = node_end_line(v).max(key_line);
                found = Some((
                    EntrySpan { key_line, indent, end_line, inline: value_start == key_line },
                    v.clone(),
                ));
                break;
            }
        }
        match found {
            Some((span, v)) => {
                result = Some(span);
                if i + 1 < chain.len() {
                    current = v;
                }
            }
            None => return Ok(None),
        }
    }
    Ok(result)
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn needs_quoting(value: &str) -> bool {
    value.is_empty()
        || value.contains(['#', ':', '{', '}', '[', ']', ',', '&', '*', '\'', '"'])
        || value.starts_with([' ', '-', '?', '!', '%', '@', '`'])
        || value.ends_with(' ')
}

pub fn yaml_scalar(value: &str) -> String {
    if needs_quoting(value) {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Set (or insert) a scalar field on the mapping addressed by `chain`.
/// Preserves an inline `# comment` after the value. Handles one-line flow
/// mappings (`web: { tech: React }`) textually.
pub fn set_field(text: &str, chain: &[&str], field: &str, value: &str) -> Result<String, String> {
    let scalar = yaml_scalar(value);
    let lines = lines_of(text);

    // Empty chain = the file's root mapping (a system file's own fields —
    // renaming a system lands here). Replace in place, or insert after the
    // last root-level scalar so the header block reads as hand-written.
    if chain.is_empty() {
        if let Some(f) = find_entry(text, &[field])? {
            return Ok(replace_field_line(&lines, f.key_line - 1, field, &scalar));
        }
        let root = parse(text)?;
        let map = as_map(&root).ok_or("root is not a mapping")?;
        let mut after_line = 1;
        for (k, v) in map.iter() {
            if matches!(v, marked_yaml::Node::Scalar(_)) {
                if let Some(m) = k.span().start() {
                    after_line = after_line.max(m.line());
                }
            }
        }
        let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        out.insert(after_line, format!("{field}: {scalar}\n"));
        return Ok(out.concat());
    }

    let owner = find_entry(text, chain)?.ok_or_else(|| format!("{chain:?} not found"))?;

    if owner.inline {
        // one-line flow mapping: edit within the braces on the key line
        let idx = owner.key_line - 1;
        let line = lines[idx];
        let eol = line.trim_end_matches(['\r', '\n']);
        let nl = &line[eol.len()..];
        let new_line = if let Some((a, b)) = flow_field_pos(eol, field) {
            // field exists inside the braces: replace it in place
            format!("{}{field}: {scalar}{}{nl}", &eol[..a], &eol[b..])
        } else if let Some(open) = eol.find('{') {
            // insert as the first field inside the braces
            let rest = eol[open + 1..].trim_start();
            let pad_removed = eol.len() - open - 1 - rest.len();
            let insert = if rest.starts_with('}') {
                format!(" {field}: {scalar} ")
            } else {
                format!(" {field}: {scalar}, ")
            };
            format!("{}{}{}{}{nl}", &eol[..open + 1], insert, &eol[open + 1 + pad_removed..], "")
        } else {
            // scalar-valued entry (no mapping): convert to block
            let indent = " ".repeat(owner.indent);
            let key = chain.last().unwrap();
            format!("{indent}{key}:\n{indent}  {field}: {scalar}{nl}")
        };
        let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        out[idx] = new_line;
        return Ok(out.concat());
    }

    // block mapping: does the field exist?
    let mut sub = chain.to_vec();
    sub.push(field);
    if let Some(f) = find_entry(text, &sub)? {
        Ok(replace_field_line(&lines, f.key_line - 1, field, &scalar))
    } else {
        // insert as first line under the owner key
        let child_indent = child_indent_for(&lines, owner);
        let insert = format!("{}{}: {}\n", " ".repeat(child_indent), field, scalar);
        let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        out.insert(owner.key_line, insert); // after the key line (1-based -> index)
        Ok(out.concat())
    }
}

/// Replace the value on an existing `field:` line, preserving indentation
/// and any trailing comment.
fn replace_field_line(lines: &[&str], idx: usize, field: &str, scalar: &str) -> String {
    let line = lines[idx];
    let eol = line.trim_end_matches(['\r', '\n']);
    let nl = &line[eol.len()..];
    let key_pat = format!("{field}:");
    let kpos = eol.find(&key_pat).unwrap_or(0);
    let after = &eol[kpos + key_pat.len()..];
    let comment = after.find(" #").map(|p| &after[p..]).unwrap_or("");
    let new_line = format!("{}{} {}{}{}", &eol[..kpos], key_pat, scalar, comment, nl);
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    out[idx] = new_line;
    out.concat()
}

/// Position of `field: value` inside a one-line flow mapping, as byte range.
fn flow_field_pos(line: &str, field: &str) -> Option<(usize, usize)> {
    let open = line.find('{')?;
    let close = line.rfind('}')?;
    let inner = &line[open + 1..close];
    let pat = format!("{field}:");
    let rel = inner.find(&pat)?;
    let start = open + 1 + rel;
    let after = &line[start + pat.len()..close];
    let len = after.find(',').unwrap_or(after.len());
    Some((start, start + pat.len() + len))
}

/// Indent for children of an entry: existing child's indent, else key + 2.
fn child_indent_for(lines: &[&str], owner: EntrySpan) -> usize {
    for line in lines.iter().skip(owner.key_line).take(owner.end_line - owner.key_line) {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        return line.len() - line.trim_start().len();
    }
    owner.indent + 2
}

/// Insert a new mapping entry (block form) under `chain` (which addresses the
/// parent mapping key, e.g. ["containers"]). Creates the parent section at
/// `create_under` when missing. Returns the new text.
pub fn insert_entry(
    text: &str,
    chain: &[&str],
    create_under: Option<(&[&str], usize)>, // (owner chain, child indent) when chain missing
    id: &str,
    fields: &[(&str, &str)],
) -> Result<String, String> {
    let lines = lines_of(text);
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    let (insert_at, entry_indent) = match find_entry(text, chain)? {
        Some(section) => {
            let indent = child_indent_for(&lines, section);
            (section.end_line, indent) // after the section's last line
        }
        None => {
            let (owner_chain, section_indent) = create_under.ok_or("section missing")?;
            let section_key = chain.last().ok_or("empty chain")?;
            let at = if owner_chain.is_empty() {
                lines.len()
            } else {
                find_entry(text, owner_chain)?.ok_or("owner not found")?.end_line
            };
            out.insert(at, format!("{}{}:\n", " ".repeat(section_indent), section_key));
            (at + 1, section_indent + 2)
        }
    };

    let ind = " ".repeat(entry_indent);
    let find = " ".repeat(entry_indent + 2);
    let mut block = format!("{ind}{id}:\n");
    for (k, v) in fields {
        block.push_str(&format!("{find}{k}: {}\n", yaml_scalar(v)));
    }
    out.insert(insert_at, block);
    Ok(out.concat())
}

/// Remove the entry addressed by `chain` — its key line through its value end.
pub fn remove_entry(text: &str, chain: &[&str]) -> Result<String, String> {
    let span = find_entry(text, chain)?.ok_or_else(|| format!("{chain:?} not found"))?;
    let lines = lines_of(text);
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        if n < span.key_line || n > span.end_line {
            out.push_str(line);
        }
    }
    Ok(out)
}

/// Append an item to the block sequence at `chain` (creating the section at
/// document/owner end when absent). `fields` render as `- k: v` + indented ks.
pub fn append_seq_item(
    text: &str,
    chain: &[&str],
    owner_chain_if_missing: (&[&str], usize),
    fields: &[(&str, &str)],
) -> Result<String, String> {
    let lines = lines_of(text);
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    let (insert_at, item_indent) = match find_entry(text, chain)? {
        Some(section) => {
            // existing item indent, else section indent + 2
            let mut indent = section.indent + 2;
            for line in lines.iter().skip(section.key_line).take(section.end_line - section.key_line) {
                let t = line.trim_start();
                if t.starts_with('-') {
                    indent = line.len() - t.len();
                    break;
                }
            }
            (section.end_line, indent)
        }
        None => {
            let (owner_chain, section_indent) = owner_chain_if_missing;
            let section_key = chain.last().ok_or("empty chain")?;
            let at = if owner_chain.is_empty() {
                lines.len()
            } else {
                find_entry(text, owner_chain)?.ok_or("owner not found")?.end_line
            };
            out.insert(at, format!("{}{}:\n", " ".repeat(section_indent), section_key));
            (at + 1, section_indent + 2)
        }
    };

    let ind = " ".repeat(item_indent);
    let cont = " ".repeat(item_indent + 2);
    let mut block = String::new();
    for (i, (k, v)) in fields.iter().enumerate() {
        if i == 0 {
            block.push_str(&format!("{ind}- {k}: {}\n", yaml_scalar(v)));
        } else {
            block.push_str(&format!("{cont}{k}: {}\n", yaml_scalar(v)));
        }
    }
    out.insert(insert_at, block);
    Ok(out.concat())
}

/// Remove sequence items under `chain` for which `pred` (given the item's
/// mapping) returns true. Returns (new text, removed count).
pub fn remove_seq_items(
    text: &str,
    chain: &[&str],
    pred: impl Fn(&MarkedMappingNode) -> bool,
) -> Result<(String, usize), String> {
    let root = parse(text)?;
    // walk to the sequence
    let mut current = root;
    for key in chain {
        let map = as_map(&current).ok_or("not a mapping")?;
        let mut next = None;
        for (k, v) in map.iter() {
            if k.as_str() == *key {
                next = Some(v.clone());
                break;
            }
        }
        current = match next {
            Some(v) => v,
            None => return Ok((text.to_string(), 0)),
        };
    }
    let Node::Sequence(seq) = &current else {
        return Ok((text.to_string(), 0));
    };

    let mut drop_ranges: Vec<(usize, usize)> = Vec::new();
    for item in seq.iter() {
        if let Node::Mapping(m) = item {
            if pred(m) {
                let start = m
                    .span()
                    .start()
                    .map(|mk| mk.line())
                    .ok_or("item without marker")?;
                let end = node_end_line(item);
                drop_ranges.push((start, end));
            }
        }
    }
    if drop_ranges.is_empty() {
        return Ok((text.to_string(), 0));
    }
    let count = drop_ranges.len();
    let lines = lines_of(text);
    let mut out = String::new();
    'outer: for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        for (s, e) in &drop_ranges {
            if n >= *s && n <= *e {
                continue 'outer;
            }
        }
        out.push_str(line);
    }
    Ok((out, count))
}
