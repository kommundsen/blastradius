The model viewport. `.canvas` clips and paints the ground; the inner `.canvas-camera`
carries the transform, the dot grid, and everything that is part of the drawing.

```jsx
<Canvas
  scale={zoom} x={pan.x} y={pan.y}
  style={{ height: 480 }}
  overlay={<ButtonGroup>…zoom…</ButtonGroup>}
>
  <EdgeLayer>
    <Edge from={{x:216,y:150}} to={{x:300,y:196}} label="JSON / HTTPS" />
  </EdgeLayer>
  <DiagramNode type="container" kicker="Container - Go" title="API Application" x={300} y={160} width={160} />
</Canvas>
```

The zoom rule: children of the camera scale with the model. `overlay` is screen-space
chrome and never scales. Level changes (L1<->L4) animate the camera transform with
`--transition-camera`; nothing crossfades and nothing jumps.

Set `theme` only to pin a subtree against the app theme (e.g. a light export preview
inside a dark app). Otherwise omit it and inherit.
