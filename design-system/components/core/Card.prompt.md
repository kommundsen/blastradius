Framed content block with four registration marks. Transparent — no surface fill.

```jsx
<Card kicker="Container" title="API Application" meta="Go - 8 components">
  Handles all authentication and transaction routing.
</Card>
```

Do not add `.duotone` to a Card. The registration marks straddle the border at -6px
and any clipping ancestor eats them; wrap the image in its own `.duotone` figure inside.
