# Sharing and export

Everything is generated locally. Nothing leaves the machine, and there is no
hosting to sign up for.

## Self-contained HTML

**Share** produces a single `.html` file with the model, the fonts, the layout
engine, and the viewer all embedded. It works from `file://`, offline, forever,
and makes no network requests at all.

The recipient gets the real thing rather than a picture: they can fly between
altitudes, dive, inspect elements and relations, and read attached documents.

You choose whether to include document **bodies**. Structure is usually safe to
pass around; the prose in your ADRs may not be. Off by default.

Headless, for CI or a script:

```
blastradius export . -o architecture.html
```

## Images

Share also exports the current view as **SVG** or **PNG** — for slides, a
README, or a ticket. SVG stays crisp and is searchable; PNG pastes anywhere.

## Rendering in CI

`tools/render-views.mjs` renders every declared view headlessly, through the
same layout and SVG code the app uses, so a CI render matches what you see.
Output is byte-stable: identical model, identical bytes, so it is safe to
commit or diff.

A useful pattern is a bot that comments on every model-touching pull request
with the semantic diff and before/after renders, so reviewers who will not
open the app still see what changed.

## Why not a hosted link

Hosted share links are a possible future, not a current feature. The core is
local-first on purpose: the repo is the database, and an export you can email
is a stronger guarantee than a URL that might stop resolving.
