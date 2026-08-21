Centred blueprint panel over a scrim. `role="dialog"`, `aria-modal`, labelled by its
title. Wire Escape and focus-trapping at the call site; the component does not own
focus management.

```jsx
<Dialog title="Discard changes?" onDismiss={close}
  actions={<><Button onClick={close}>Cancel</Button>
             <Button variant="danger" onClick={discard}>Discard</Button></>}>
  3 uncommitted edits to model.yaml will be lost.
</Dialog>
```
