//! Self-contained HTML export (ADR-0009, spec/export.md): one file, zero
//! network requests, works from file://. Headless by construction — layout
//! runs in the viewer at open time (elkjs is embedded), so this module is
//! pure asset assembly and needs no WebView.
//!
//! Assets are embedded at compile time from ui/ (the same modules the app
//! runs — the export IS the mock-harness contract of ADR-0011, sealed), and
//! the snapshot is the exact shape that becomes the v2 hosted-link payload.

use crate::diagnostics::Diagnostic;
use crate::model::Workspace;
use crate::vfs::Vfs;

const DS_TOKENS_COLORS: &str = include_str!("../../../ui/ds/tokens/colors.css");
const DS_TOKENS_TYPE: &str = include_str!("../../../ui/ds/tokens/typography.css");
const DS_TOKENS_SPACING: &str = include_str!("../../../ui/ds/tokens/spacing.css");
const DS_TOKENS_LAYOUT: &str = include_str!("../../../ui/ds/tokens/layout.css");
const DS_TOKENS_MOTION: &str = include_str!("../../../ui/ds/tokens/motion.css");
const DS_BASE: &str = include_str!("../../../ui/ds/foundations/base.css");
const DS_COMPONENTS: &str = include_str!("../../../ui/ds/components/components.css");
const APP_CSS: &str = include_str!("../../../ui/app.css");
const ELK_JS: &str = include_str!("../../../ui/vendor/elk.bundled.js");
const MARKED_JS: &str = include_str!("../../../ui/vendor/marked.min.js");
const DATA_JS: &str = include_str!("../../../ui/js/data.js");
const LAYOUT_JS: &str = include_str!("../../../ui/js/layout.js");
const VIEWER_JS: &str = include_str!("../../../ui/js/viewer.js");

/// (family, weight, subset marker, bytes)
const FONTS: &[(&str, u32, &str, &[u8])] = &[
    ("Barlow", 400, "latin", include_bytes!("../../../ui/ds/assets/fonts/barlow-400-latin.woff2")),
    ("Barlow", 500, "latin", include_bytes!("../../../ui/ds/assets/fonts/barlow-500-latin.woff2")),
    ("Barlow", 700, "latin", include_bytes!("../../../ui/ds/assets/fonts/barlow-700-latin.woff2")),
    ("Barlow Condensed", 400, "latin", include_bytes!("../../../ui/ds/assets/fonts/barlow-condensed-400-latin.woff2")),
    ("Barlow Condensed", 600, "latin", include_bytes!("../../../ui/ds/assets/fonts/barlow-condensed-600-latin.woff2")),
];

pub struct ExportOptions {
    /// Include full markdown bodies of linked documents (they may be more
    /// sensitive than structure — spec/export.md).
    pub include_doc_bodies: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { include_doc_bodies: false }
    }
}

fn base64(data: &[u8]) -> String {
    // Minimal encoder — not worth a dependency for five font files.
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

fn font_faces() -> String {
    FONTS
        .iter()
        .map(|(family, weight, _, bytes)| {
            format!(
                "@font-face{{font-family:'{family}';font-style:normal;font-weight:{weight};\
                 font-display:swap;src:url(data:font/woff2;base64,{}) format('woff2')}}\n",
                base64(bytes)
            )
        })
        .collect()
}

/// Strip `export ` prefixes so ES modules concatenate into one classic script.
fn strip_exports(src: &str) -> String {
    // Top-level `export ` prefixes only — our modules never nest exports.
    src.lines()
        .map(|l| l.strip_prefix("export ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("
")
}

/// Build the single-file interactive HTML for a workspace.
pub fn export_html(
    vfs: &dyn Vfs,
    ws: &Workspace,
    diags: &[Diagnostic],
    options: &ExportOptions,
) -> Result<String, String> {
    let mut snap = crate::snapshot::snapshot(vfs, ws, diags);
    if !options.include_doc_bodies {
        for d in &mut snap.docs {
            d.body = String::new();
        }
    }
    let snapshot_json = serde_json::to_string(&snap).map_err(|e| e.to_string())?;
    // </script> inside JSON strings would terminate our script block
    let snapshot_json = snapshot_json.replace("</", "<\\/");

    let css = format!(
        "{fonts}{c}{t}{s}{l}{m}{b}{comp}{app}",
        fonts = font_faces(),
        c = DS_TOKENS_COLORS,
        t = DS_TOKENS_TYPE,
        s = DS_TOKENS_SPACING,
        l = DS_TOKENS_LAYOUT,
        m = DS_TOKENS_MOTION,
        b = DS_BASE,
        comp = DS_COMPONENTS,
        app = APP_CSS,
    );

    let modules = format!("{}\n{}", strip_exports(DATA_JS), strip_exports(LAYOUT_JS));

    let name = html_escape(&ws.name);
    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{name} — Blastradius</title>
<style>{css}</style>
</head>
<body>
<div class="app" id="app">
  <header class="app-bar">
    <span class="nav-brand" style="margin-right:0;font-size:var(--text-ui)">■ Blastradius</span>
    <span class="app-bar-title text-muted" id="breadcrumb">{name}</span>
    <span class="seg" role="radiogroup" aria-label="Detail level" id="level-seg">
      <label class="seg-opt"><input type="radio" name="lvl" value="L1">L1</label>
      <label class="seg-opt"><input type="radio" name="lvl" value="L2">L2</label>
      <label class="seg-opt"><input type="radio" name="lvl" value="L3">L3</label>
      <label class="seg-opt is-disabled"><input type="radio" name="lvl" value="L4" disabled>L4</label>
    </span>
    <span class="app-bar-spacer"></span>
    <button class="btn btn-secondary" id="theme-btn">Theme: auto</button>
  </header>
  <div class="app-body" data-nav="open" data-side="open">
    <nav class="panel panel-nav"><div class="panel-body" id="tree"></div></nav>
    <div class="canvas" id="canvas" tabindex="0" aria-label="Model canvas">
      <div class="canvas-camera" id="camera" style="--camera-scale:1">
        <svg class="edge-layer" id="edge-layer" aria-hidden="true">
          <defs>
            <marker id="br-arrow" viewBox="0 0 10 10" refX="9.5" refY="5"
                    markerWidth="8" markerHeight="8" orient="auto-start-reverse" markerUnits="strokeWidth">
              <path class="edge-arrow" d="M1.5,1.5 L9,5 L1.5,8.5"/>
            </marker>
          </defs>
          <g id="edges"></g>
        </svg>
        <div id="nodes"></div>
      </div>
      <div class="canvas-overlay">
        <div class="overlay-bl">
          <span class="btn-group">
            <button class="btn" id="zoom-out" aria-label="Zoom out">−</button>
            <button class="btn" id="zoom-reset">100%</button>
            <button class="btn" id="zoom-in" aria-label="Zoom in">+</button>
          </span>
          <span class="tag tag-accent">Double-click to dive · Esc to rise</span>
        </div>
        <div style="position:absolute;right:var(--space-4);bottom:var(--space-4)">
          <span class="tag tag-neutral">made with Blastradius</span>
        </div>
      </div>
    </div>
    <aside class="panel panel-side">
      <div class="panel-head">
        <span class="panel-title">Inspector</span>
        <button class="btn btn-ghost" id="side-back" hidden>← back</button>
      </div>
      <div class="panel-body" id="side-body"></div>
    </aside>
  </div>
</div>
<script>{elk}</script>
<script>{marked}</script>
<script>
const SNAPSHOT = {snapshot_json};
const INCLUDE_DOC_BODIES = {bodies};
</script>
<script>
{modules}
{viewer}
</script>
</body>
</html>
"##,
        elk = ELK_JS,
        marked = MARKED_JS,
        bodies = options.include_doc_bodies,
        viewer = VIEWER_JS,
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
