/* @ds-bundle: {"format":4,"namespace":"BlastradiusDesignSystem_de4189","components":[{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"ButtonGroup","sourcePath":"components/core/Button.jsx"},{"name":"Card","sourcePath":"components/core/Card.jsx"},{"name":"Dialog","sourcePath":"components/core/Dialog.jsx"},{"name":"Input","sourcePath":"components/core/Input.jsx"},{"name":"Segmented","sourcePath":"components/core/Segmented.jsx"},{"name":"Tag","sourcePath":"components/core/Tag.jsx"},{"name":"Canvas","sourcePath":"components/diagram/Canvas.jsx"},{"name":"DiagramNode","sourcePath":"components/diagram/DiagramNode.jsx"},{"name":"Edge","sourcePath":"components/diagram/Edge.jsx"},{"name":"EdgeLayer","sourcePath":"components/diagram/EdgeLayer.jsx"}],"sourceHashes":{"components/core/Button.jsx":"bf5309340e86","components/core/Card.jsx":"e1e11cac8d5b","components/core/Dialog.jsx":"53827dcef481","components/core/Input.jsx":"49130550e33a","components/core/Segmented.jsx":"4ba89069ed49","components/core/Tag.jsx":"bf6e79e710b7","components/diagram/Canvas.jsx":"ecec874e41c6","components/diagram/DiagramNode.jsx":"3316ab15ad97","components/diagram/Edge.jsx":"a49e7cc8ffc1","components/diagram/EdgeLayer.jsx":"fcf7a0cfed8d"},"inlinedExternals":[],"unexposedExports":[]} */


(() => {

const __ds_ns = (window.BlastradiusDesignSystem_de4189 = window.BlastradiusDesignSystem_de4189 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function Button({
  variant = 'secondary',
  icon,
  block,
  children,
  ...rest
}) {
  const cls = ['btn', 'btn-' + variant, block ? 'btn-block' : ''].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("button", _extends({
    type: "button",
    className: cls
  }, rest), icon, children);
}

// Grouped actions — zoom −/100%/+, alignment, undo/redo. NOT Segmented: these are
// three independent commands, not one selected value.
function ButtonGroup({
  children,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("span", _extends({
    className: "btn-group"
  }, rest), children);
}
Object.assign(__ds_scope, { Button, ButtonGroup });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/core/Card.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function Card({
  kicker,
  title,
  meta,
  blueprint = true,
  children,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("div", _extends({
    className: blueprint ? 'card blueprint' : 'card'
  }, rest), blueprint && /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("i", {
    className: "corner tl"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner tr"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner bl"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner br"
  })), kicker && /*#__PURE__*/React.createElement("span", {
    className: "card-kicker"
  }, kicker), title && /*#__PURE__*/React.createElement("span", {
    className: "card-title"
  }, title), children && /*#__PURE__*/React.createElement("p", {
    className: "card-body"
  }, children), meta && /*#__PURE__*/React.createElement("span", {
    className: "card-meta"
  }, meta));
}
Object.assign(__ds_scope, { Card });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Card.jsx", error: String((e && e.message) || e) }); }

// components/core/Dialog.jsx
try { (() => {
let uid = 0;
function Dialog({
  title,
  actions,
  children,
  onDismiss
}) {
  const titleId = 'dlg' + ++uid;
  return /*#__PURE__*/React.createElement("div", {
    className: "dialog-backdrop",
    onClick: onDismiss
  }, /*#__PURE__*/React.createElement("div", {
    className: "dialog blueprint",
    role: "dialog",
    "aria-modal": "true",
    "aria-labelledby": title ? titleId : undefined,
    onClick: e => e.stopPropagation()
  }, /*#__PURE__*/React.createElement("i", {
    className: "corner tl"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner tr"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner bl"
  }), /*#__PURE__*/React.createElement("i", {
    className: "corner br"
  }), title && /*#__PURE__*/React.createElement("span", {
    className: "dialog-title",
    id: titleId
  }, title), /*#__PURE__*/React.createElement("div", {
    className: "dialog-body"
  }, children), actions && /*#__PURE__*/React.createElement("div", {
    className: "dialog-actions"
  }, actions)));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/core/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
let uid = 0;
function Input({
  label,
  multiline,
  error,
  id,
  ...rest
}) {
  const fieldId = id || 'in' + ++uid;
  const errId = fieldId + '-err';
  const cls = 'input' + (error ? ' is-invalid' : '');
  const control = multiline ? /*#__PURE__*/React.createElement("textarea", _extends({
    id: fieldId,
    className: cls,
    "aria-invalid": error ? true : undefined,
    "aria-describedby": error ? errId : undefined
  }, rest)) : /*#__PURE__*/React.createElement("input", _extends({
    id: fieldId,
    className: cls,
    "aria-invalid": error ? true : undefined,
    "aria-describedby": error ? errId : undefined
  }, rest));
  if (!label && !error) return control;
  return /*#__PURE__*/React.createElement("div", {
    className: "field"
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: fieldId
  }, label), control, error && /*#__PURE__*/React.createElement("span", {
    id: errId,
    className: "field-error",
    role: "alert"
  }, error));
}
Object.assign(__ds_scope, { Input });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Input.jsx", error: String((e && e.message) || e) }); }

// components/core/Segmented.jsx
try { (() => {
// Single-select. Real radios in a named group, so arrow keys, screen readers and
// form semantics all work — the previous version was click-only <label>s with no
// input, which made the L1-L4 level switcher unreachable by keyboard.
function Segmented({
  options,
  value,
  onChange,
  name = 'seg',
  label
}) {
  return /*#__PURE__*/React.createElement("span", {
    className: "seg",
    role: "radiogroup",
    "aria-label": label
  }, options.map(o => /*#__PURE__*/React.createElement("label", {
    key: o,
    className: 'seg-opt' + (o === value ? ' is-active' : '')
  }, /*#__PURE__*/React.createElement("input", {
    type: "radio",
    name: name,
    value: o,
    checked: o === value,
    onChange: () => onChange && onChange(o)
  }), o)));
}
Object.assign(__ds_scope, { Segmented });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Segmented.jsx", error: String((e && e.message) || e) }); }

// components/core/Tag.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function Tag({
  variant = 'neutral',
  children,
  ...rest
}) {
  return /*#__PURE__*/React.createElement("span", _extends({
    className: 'tag tag-' + variant
  }, rest), children);
}
Object.assign(__ds_scope, { Tag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tag.jsx", error: String((e && e.message) || e) }); }

// components/diagram/Canvas.jsx
try { (() => {
function Canvas({
  theme,
  scale = 1,
  x = 0,
  y = 0,
  overlay,
  style,
  children
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "canvas",
    "data-theme": theme,
    style: style
  }, /*#__PURE__*/React.createElement("div", {
    className: "canvas-camera",
    style: {
      // JS owns exactly two things: this transform, and --camera-scale matching it.
      transform: `translate(${x}px, ${y}px) scale(${scale})`,
      '--camera-scale': scale,
      // Below 0.5x the 26px dot pitch collapses into a wash — coarsen it instead.
      '--canvas-dot-pitch': (scale < 0.5 ? 104 : 26) + 'px'
    }
  }, children), overlay && /*#__PURE__*/React.createElement("div", {
    className: "canvas-overlay"
  }, overlay));
}
Object.assign(__ds_scope, { Canvas });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/diagram/Canvas.jsx", error: String((e && e.message) || e) }); }

// components/diagram/DiagramNode.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
// A C4 element. `type` is the C4 kind (person / system / container / component);
// `status` is git/validation state. They are independent axes — a container can be
// added, a person can be invalid — so they are separate props, not one enum.
//
// Type is encoded by geometry and status by colour+glyph, never colour alone:
// a node must stay legible in greyscale and at L1, where it is ~90px wide.

const BADGE = {
  added: '+',
  removed: '−',
  changed: '~',
  conflict: '!',
  invalid: '!'
};
const BADGE_LABEL = {
  added: 'Added in this change',
  removed: 'Removed in this change',
  changed: 'Modified in this change',
  conflict: 'Merge conflict',
  invalid: 'Invalid — see model errors'
};
function DiagramNode({
  kicker,
  title,
  meta,
  type = 'system',
  status,
  active,
  external,
  x,
  y,
  width,
  style,
  onSelect,
  ...rest
}) {
  const cls = ['node', 'is-' + type, external ? 'is-external' : '', active ? 'is-active' : '', status ? 'is-' + status : ''].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("div", _extends({
    className: cls,
    style: {
      left: x,
      top: y,
      width,
      ...style
    }
    // The core object of the app is keyboard-reachable and announces its state.
    ,
    tabIndex: 0,
    role: "button",
    "aria-pressed": active ? true : undefined,
    onClick: onSelect,
    onKeyDown: onSelect && (e => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onSelect(e);
      }
    })
  }, rest), status && /*#__PURE__*/React.createElement("span", {
    className: "node-badge",
    title: BADGE_LABEL[status]
  }, /*#__PURE__*/React.createElement("span", {
    "aria-hidden": "true"
  }, BADGE[status]), /*#__PURE__*/React.createElement("span", {
    className: "sr-only"
  }, BADGE_LABEL[status])), kicker && /*#__PURE__*/React.createElement("span", {
    className: "node-kicker"
  }, kicker), /*#__PURE__*/React.createElement("span", {
    className: "node-title"
  }, title), meta && /*#__PURE__*/React.createElement("span", {
    className: "node-meta"
  }, meta));
}
Object.assign(__ds_scope, { DiagramNode });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/diagram/DiagramNode.jsx", error: String((e && e.message) || e) }); }

// components/diagram/Edge.jsx
try { (() => {
// A C4 relationship. Directed by default, because "Web App uses API" is not the same
// statement as "API uses Web App" — the previous rotated-<i> edge could not say either.
//
// Geometry is given as real points, not (x, length, angle): the router owns the shape,
// the component owns the drawing. routing="orthogonal" elbows through the midpoint,
// which is what C4 container diagrams normally want.

function buildPoints(from, to, waypoints, routing) {
  if (waypoints && waypoints.length) return [from, ...waypoints, to];
  if (routing === 'orthogonal') {
    const mx = (from.x + to.x) / 2;
    return [from, {
      x: mx,
      y: from.y
    }, {
      x: mx,
      y: to.y
    }, to];
  }
  return [from, to];
}

// Midpoint by arc length, so the label sits on the visual middle of an elbowed run.
function midpoint(pts) {
  const seg = [];
  let total = 0;
  for (let i = 1; i < pts.length; i++) {
    const len = Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
    seg.push(len);
    total += len;
  }
  let want = total / 2;
  for (let i = 0; i < seg.length; i++) {
    if (want <= seg[i]) {
      const t = seg[i] ? want / seg[i] : 0;
      return {
        x: pts[i].x + (pts[i + 1].x - pts[i].x) * t,
        y: pts[i].y + (pts[i + 1].y - pts[i].y) * t
      };
    }
    want -= seg[i];
  }
  return pts[pts.length - 1];
}
function Edge({
  from,
  to,
  waypoints,
  routing = 'straight',
  direction = 'forward',
  secondary,
  active,
  status,
  label,
  labelOffset = -4,
  onSelect
}) {
  const pts = buildPoints(from, to, waypoints, routing);
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p.x + ',' + p.y).join(' ');
  const mid = label ? midpoint(pts) : null;
  const cls = ['edge', direction === 'both' ? 'is-bidirectional' : '', direction === 'none' ? 'is-undirected' : '', secondary ? 'is-secondary' : '', active ? 'is-active' : '', status ? 'is-' + status : ''].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("g", null, onSelect && /*#__PURE__*/React.createElement("path", {
    className: "edge-hit",
    d: d,
    onClick: onSelect
  }), /*#__PURE__*/React.createElement("path", {
    className: cls,
    d: d
  }), label && /*#__PURE__*/React.createElement("text", {
    className: "edge-label",
    x: mid.x,
    y: mid.y + labelOffset,
    textAnchor: "middle"
  }, label));
}
Object.assign(__ds_scope, { Edge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/diagram/Edge.jsx", error: String((e && e.message) || e) }); }

// components/diagram/EdgeLayer.jsx
try { (() => {
// The SVG surface every <Edge> must live inside. It carries the arrow marker in its
// own <defs> — SVG markers are resolved per document fragment, so one shared marker
// in some far-off <svg> will not work. Render exactly one EdgeLayer per Canvas.
function EdgeLayer({
  children
}) {
  return /*#__PURE__*/React.createElement("svg", {
    className: "edge-layer",
    "aria-hidden": "true"
  }, /*#__PURE__*/React.createElement("defs", null, /*#__PURE__*/React.createElement("marker", {
    id: "br-arrow",
    viewBox: "0 0 10 10",
    refX: "9.5",
    refY: "5",
    markerWidth: "8",
    markerHeight: "8",
    orient: "auto-start-reverse",
    markerUnits: "strokeWidth"
  }, /*#__PURE__*/React.createElement("path", {
    className: "edge-arrow",
    d: "M1.5,1.5 L9,5 L1.5,8.5"
  }))), children);
}
Object.assign(__ds_scope, { EdgeLayer });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/diagram/EdgeLayer.jsx", error: String((e && e.message) || e) }); }

__ds_ns.Button = __ds_scope.Button;

__ds_ns.ButtonGroup = __ds_scope.ButtonGroup;

__ds_ns.Card = __ds_scope.Card;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Segmented = __ds_scope.Segmented;

__ds_ns.Tag = __ds_scope.Tag;

__ds_ns.Canvas = __ds_scope.Canvas;

__ds_ns.DiagramNode = __ds_scope.DiagramNode;

__ds_ns.Edge = __ds_scope.Edge;

__ds_ns.EdgeLayer = __ds_scope.EdgeLayer;

})();
