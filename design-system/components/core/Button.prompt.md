Square-cornered action button; primary is the single solid accent fill on the board.

```jsx
<Button variant="primary">Share</Button>
<Button>Cancel</Button>
<Button variant="ghost">Reply</Button>
<Button variant="danger">Discard changes</Button>
```

Icon buttons: pass a 16px Lucide icon via `icon` (or as only child with .btn-icon styling for square 36px).

Grouped independent commands use `ButtonGroup`, never `Segmented`:

```jsx
<ButtonGroup>
  <Button aria-label="Zoom out">-</Button>
  <Button>100%</Button>
  <Button aria-label="Zoom in">+</Button>
</ButtonGroup>
```
