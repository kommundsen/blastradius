//! View files (spec §4): scope, level, pinned layout in grid units.

use crate::diagnostics::Diagnostic;
use crate::model::{View, Workspace};
use crate::vfs::Vfs;
use crate::yaml;
use marked_yaml::Node;
use std::collections::BTreeMap;

pub fn parse_view_file(vfs: &dyn Vfs, rel: &str, ws: &mut Workspace, diags: &mut Vec<Diagnostic>) {
    let Some((node, _)) = yaml::load_file(vfs, rel, diags) else {
        return;
    };
    let Some(map) = yaml::as_mapping(&node, rel, "view file", diags) else {
        return;
    };

    let Some(id) = yaml::get_str(map, "view").map(str::to_string) else {
        diags.push(Diagnostic::error(rel, 1, "view file needs `view:` id"));
        return;
    };
    let line = yaml::field_line(map, "view");

    let Some(scope) = yaml::get_str(map, "scope").map(str::to_string) else {
        diags.push(Diagnostic::error(rel, line, "view needs `scope:`"));
        return;
    };

    let level = yaml::get_str(map, "level").unwrap_or("").to_string();
    if !matches!(level.as_str(), "L1" | "L2" | "L3") {
        diags.push(Diagnostic::error(
            rel,
            yaml::field_line(map, "level"),
            format!("bad level {level:?} — expected L1, L2, or L3 (spec §4)"),
        ));
    }

    let mut layout = BTreeMap::new();
    if let Some(Node::Mapping(pins)) = map.get_node("layout") {
        for (key, val) in pins.iter() {
            let pin_id = key.as_str().to_string();
            let pin_line = yaml::scalar_line(key);
            let pos = yaml::as_sequence(val).and_then(|seq| {
                let nums: Vec<f64> = seq
                    .iter()
                    .filter_map(yaml::as_str)
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                (nums.len() == 2 && seq.iter().count() == 2).then(|| (nums[0], nums[1]))
            });
            match pos {
                Some(xy) => {
                    // Pin targets resolve against scope in validate.
                    layout.insert(pin_id, xy);
                }
                None => diags.push(Diagnostic::error(
                    rel,
                    pin_line,
                    format!("layout {pin_id:?} must be [x, y] grid units"),
                )),
            }
        }
    }

    ws.views.push(View {
        id,
        name: yaml::get_str(map, "name").map(str::to_string),
        scope,
        level,
        layout,
        include_context: yaml::get_str(map, "include-context") != Some("false"),
        file: rel.to_string(),
        line,
    });
}
