Square field on `--color-surface`. Pass `label` for the `.field` wrapper; the label is
associated by id, not by proximity.

```jsx
<Input label="Model name" placeholder="internet-banking" />
<Input label="Repository" value={url} onChange={e => setUrl(e.target.value)}
  error="Not a git remote" />
```

`error` sets `.is-invalid`, `aria-invalid`, and `aria-describedby`, and renders a
`!`-prefixed message — the state never rests on colour alone.
