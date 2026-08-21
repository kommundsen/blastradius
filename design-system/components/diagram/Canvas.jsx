export function Canvas({ theme, scale = 1, x = 0, y = 0, overlay, style, children }) {
  return <div className="canvas" data-theme={theme} style={style}>
    <div
      className="canvas-camera"
      style={{
        // JS owns exactly two things: this transform, and --camera-scale matching it.
        transform: `translate(${x}px, ${y}px) scale(${scale})`,
        '--camera-scale': scale,
        // Below 0.5x the 26px dot pitch collapses into a wash — coarsen it instead.
        '--canvas-dot-pitch': (scale < 0.5 ? 104 : 26) + 'px',
      }}
    >{children}</div>
    {overlay && <div className="canvas-overlay">{overlay}</div>}
  </div>;
}
