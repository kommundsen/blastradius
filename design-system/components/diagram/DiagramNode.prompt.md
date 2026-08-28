A C4 element. Two independent axes: `type` is what it is, `status` is what git and the
validator say about it.

```jsx
<DiagramNode type="person" title="Customer" kicker="Person" x={40} y={60} width={140} />
<DiagramNode type="container" kicker="Container - Go" title="API Application"
  meta="8 components" x={300} y={160} width={160} active onSelect={dive} />
<DiagramNode type="system" title="Mainframe" kicker="External system" external x={420} y={380} width={150} />
<DiagramNode type="container" kicker="Container - Redis" title="Session Cache"
  status="added" x={620} y={220} width={150} />
```

Type is geometry (spine, head, fill), status is colour plus a glyph badge — never
colour alone. `onSelect` makes the node a real button: focusable, Enter/Space activated.
Position with `x`/`y`/`width` in camera coordinates, not with a `style` override.

`description` puts the element's own sentence at the bottom of the box, under a
hairline, which is where C4 writes it. It is opt-in per diagram rather than a
property of the element: most models describe nearly everything, and drawing
them all at once makes every box tall. The box grows to fit — never give it a
fixed height when a description is set.
