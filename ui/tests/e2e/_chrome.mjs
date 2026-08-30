// Shared helpers for the app chrome (ADR-0020). Not a spec — the runner picks
// up `*.spec.mjs`, so this file is imported, never executed on its own.

/**
 * Run one of the actions the bar no longer shows directly. Open, Theme, Help,
 * Diff and History live in the ⋯ menu; the bar keeps where you are, what is
 * true, what you just did, Find and Share.
 *
 * `label` is matched against the menu item's text, which is the button's own
 * label plus its shortcut — so 'Help' matches 'Help · ?'.
 */
export async function barAction(page, label) {
  await page.locator('#more-btn').click();
  await page.locator('.ctx-menu .ctx-item', { hasText: label }).first().click();
}

/** Whether the ⋯ menu offers an action at all — Diff and History exist only
 *  with a git base, and an action that does not apply is absent, not greyed. */
export async function barOffers(page, label) {
  await page.locator('#more-btn').click();
  const n = await page.locator('.ctx-menu .ctx-item', { hasText: label }).count();
  await page.keyboard.press('Escape');
  return n > 0;
}
