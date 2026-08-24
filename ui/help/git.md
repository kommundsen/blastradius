# Git: diff, history, conflicts

The model is text in your repo, so it reviews like code. Blastradius adds the
part git cannot do on its own: showing you what a change *means* rather than
which lines moved.

Blastradius never writes to your repository. It reads git state, and staging is
done through your own `git` — commits stay yours.

## Status

When the workspace is inside a repository, the toolbar shows the branch, how
far ahead or behind you are, and whether anything is conflicted.

## Semantic diff

**Diff** compares the working tree against a base you choose and paints the
result onto the canvas:

- **added** elements and relations,
- **removed** ones, drawn as ghosts so you can see what left,
- **changed** ones, where a field differs.

Every state pairs a colour with a glyph and a text label, so a diff is readable
in greyscale and to colour-blind readers.

Layout changes are kept separate from semantic ones — moving a box is not a
change to the architecture, and the diff says so. That separation is also why
positions live in `views/` rather than in model files.

## History

**History** lists commits that touched the model. Pick one and the canvas
travels to it: you are looking at the model as it was, with a banner telling
you so. Return brings you back to the working tree.

This is the fastest way to answer "what did the architecture look like when we
shipped that?"

## Conflicts

A merge conflict in a model file is flagged on the canvas: conflicted elements
are badged, and the inspector shows **ours** and **theirs** side by side for
each one, so you resolve per element rather than by picking through markers.

Choose a side for each conflicted element and apply. Blastradius rebuilds the
file from the side you chose, **validates the whole result before writing**,
and refuses invalid resolutions outright rather than leaving you with a broken
model. Comments and formatting survive. The resolution is staged through your
own `git add`; the commit is still yours to make.

Elements you do not decide keep your side.

You can equally resolve in your editor — the app notices and moves on.

## What to review in a PR

The model diff is a normal text diff, which is the point: it belongs in the
same PR as the code it describes. For a rendered before-and-after in the PR
itself, see the CI recipe in [Sharing and export](export.md).
