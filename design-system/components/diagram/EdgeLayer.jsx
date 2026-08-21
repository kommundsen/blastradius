// The SVG surface every <Edge> must live inside. It carries the arrow marker in its
// own <defs> — SVG markers are resolved per document fragment, so one shared marker
// in some far-off <svg> will not work. Render exactly one EdgeLayer per Canvas.
export function EdgeLayer({ children }) {
  return <svg className="edge-layer" aria-hidden="true">
    <defs>
      <marker
        id="br-arrow" viewBox="0 0 10 10"
        refX="9.5" refY="5" markerWidth="8" markerHeight="8"
        orient="auto-start-reverse" markerUnits="strokeWidth"
      >
        {/* Open chevron, not a filled triangle — the model is a line drawing. */}
        <path className="edge-arrow" d="M1.5,1.5 L9,5 L1.5,8.5" />
      </marker>
    </defs>
    {children}
  </svg>;
}
