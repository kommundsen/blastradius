Single-select control — one value out of a short list. Renders real radios, so arrow
keys and screen readers work. The L1-L4 level switcher is the canonical use.

```jsx
<Segmented label="Detail level" name="level"
  options={['L1','L2','L3','L4']} value={level} onChange={setLevel} />
```

Not for toolbars. Zoom -/100%/+ is three commands, not one value — use `ButtonGroup`.
