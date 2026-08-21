A C4 relationship: an SVG path with a directional arrowhead. Take points from the
layout engine — the component draws, it does not compute geometry.

```jsx
<Edge from={{x:216,y:150}} to={{x:300,y:196}} label="JSON / HTTPS" />
<Edge from={a} to={b} routing="orthogonal" direction="both" label="reads / writes" />
<Edge from={a} to={c} secondary direction="none" />
<Edge from={a} to={d} status="added" onSelect={() => select(id)} />
```

`direction` defaults to `forward` — every C4 relation has one, and an undirected edge
should be a deliberate choice. Strokes use `vector-effect: non-scaling-stroke`, so a
hairline stays one device pixel at any zoom. `onSelect` is what creates the fat
invisible hit-path; without it the edge cannot be clicked.
