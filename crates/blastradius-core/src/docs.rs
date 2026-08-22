//! Typed documents (ADR-0010, spec §5): markdown files with YAML frontmatter.
//! Bodies stay untouched — only the frontmatter joins the model.

use crate::diagnostics::Diagnostic;
use crate::model::{is_valid_slug, Doc, Workspace};
use crate::vfs::Vfs;
use crate::yaml;

pub fn parse_doc_file(vfs: &dyn Vfs, rel: &str, ws: &mut Workspace, diags: &mut Vec<Diagnostic>) {
    let text = match vfs.read(rel) {
        Ok(t) => t,
        Err(e) => {
            diags.push(Diagnostic::error(rel, 0, format!("cannot read file: {e}")));
            return;
        }
    };

    // Frontmatter = a leading `---` line, YAML until the next `---` line.
    // A matched file without frontmatter is ignored with an info notice (spec §5).
    let Some(fm_text) = extract_frontmatter(&text) else {
        diags.push(Diagnostic::info(rel, 0, "no frontmatter — ignored"));
        return;
    };

    // Frontmatter starts at line 2 (line 1 is the opening `---`).
    let Some(node) = yaml::parse_str(fm_text, rel, 1, diags) else {
        return; // malformed frontmatter is malformed YAML (spec §6) — already reported
    };
    let Some(map) = yaml::as_mapping(&node, rel, "frontmatter", diags) else {
        return;
    };

    let Some(id) = yaml::get_str(map, "doc").map(str::to_string) else {
        diags.push(Diagnostic::error(rel, 2, "frontmatter needs `doc:` id"));
        return;
    };
    let id_line = yaml::field_line(map, "doc") + 1;
    if !is_valid_slug(&id) {
        diags.push(Diagnostic::error(rel, id_line, format!("bad doc id {id:?}")));
        return;
    }

    let doc_type = yaml::get_str(map, "type").unwrap_or("note").to_string();
    let status = yaml::get_str(map, "status").map(str::to_string);

    let elements = match map.get_node("elements") {
        None => Vec::new(),
        Some(node) => match yaml::as_sequence(node) {
            Some(seq) => seq.iter().filter_map(yaml::as_str).map(str::to_string).collect(),
            None => {
                diags.push(Diagnostic::error(
                    rel,
                    yaml::line_of(node) + 1,
                    "`elements:` must be a list of element ids",
                ));
                Vec::new()
            }
        },
    };

    ws.docs.push(Doc {
        id,
        doc_type,
        status,
        elements,
        supersedes: yaml::get_str(map, "supersedes").map(str::to_string),
        file: rel.to_string(),
        line: id_line,
    });
}

fn extract_frontmatter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---")?;
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    // closing fence on its own line
    for (idx, _) in rest.match_indices("---") {
        let at_line_start = idx == 0 || rest.as_bytes().get(idx - 1) == Some(&b'\n');
        let line_end = rest[idx + 3..].trim_start_matches('\r').starts_with('\n')
            || rest[idx + 3..].is_empty();
        if at_line_start && line_end {
            return Some(&rest[..idx]);
        }
    }
    None
}
