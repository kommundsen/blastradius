# Blastradius — Privacy Policy

*Effective 2026-08-22.*

Blastradius is a local-first desktop application. This policy is short
because the app collects nothing to explain: everything it does happens on
your own machine, on files you choose to open.

## What Blastradius does not do

- **No account, no sign-in.** There is nothing to register for.
- **No data collection.** Blastradius contains no analytics, no telemetry,
  no crash reporting, and no diagnostic upload of any kind. Nothing about
  your usage, your machine, or your content is ever measured or sent
  anywhere.
- **No network access for your content.** Blastradius does not upload,
  sync, or transmit your architecture model, your files, or anything else
  you work on. It has no server component and no cloud backend.
- **No advertising, no third-party SDKs, no in-app purchases.**
- **No sharing.** Since nothing leaves your machine, there is nothing to
  share with anyone — not with us, not with a third party.

## What Blastradius stores, and where

Your architecture model is plain YAML and Markdown files, stored wherever
you choose on your own filesystem, inside your own git repository.
Blastradius reads and writes only those files, plus small local
preferences (such as window size and panel layout) stored on your device.
None of it is copied anywhere else by the app.

## Git

Blastradius reads your repository's status, history, and diffs using a
bundled, offline copy of libgit2 — entirely locally, entirely read-only.
Blastradius itself never pushes, fetches, clones, or otherwise contacts a
git remote. If you push your repository to GitHub or any other host, that
happens through your own separately installed `git`, under your own
control, outside of Blastradius.

## The MCP server (optional)

Blastradius can optionally run a local Model Context Protocol server so
coding-assistant tools (for example, Claude Code) can query your
architecture model. This server communicates only over local
inter-process I/O with a tool already running on your own machine — it
opens no network port and is not reachable from, or by, anywhere else.

## Children's privacy

Blastradius is a developer tool, not directed at children, and — as
above — it does not knowingly (or unknowingly) collect personal
information from anyone, of any age.

## Changes to this policy

If this policy ever changes, the update will be published at this same
page, versioned alongside the product in its git history.

## Contact

Questions about this policy: Kim Ommundsen — kim.ommundsen@gmail.com.
