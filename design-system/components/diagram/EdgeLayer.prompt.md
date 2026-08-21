The one SVG surface every Edge lives in. It ships the `#br-arrow` marker in its own
`<defs>`; SVG markers do not resolve across document fragments, so a second EdgeLayer
or an Edge rendered outside one will silently lose its arrowheads.

```jsx
<EdgeLayer>
  <Edge from={a} to={b} label="JSON / HTTPS" />
  <Edge from={b} to={c} routing="orthogonal" direction="both" />
</EdgeLayer>
```

One per Canvas, rendered before the nodes.
