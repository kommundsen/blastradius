# Getting started

Blastradius reads a **workspace**: a folder of plain YAML and markdown that
lives in your repo. There is no account, no server, and nothing to sync. If you
delete the app, the folder is still legible text.

## Getting the app

On Windows the **Microsoft Store** edition installs and updates itself, and is
the one to use where the Store is available.

Where it is not — a locked-down or managed desktop with the Store removed, an
air-gapped build box, or Linux — every release also publishes a **portable
archive**. Unzip it and run; no installer, no admin rights, and nothing written
outside the folder you put it in. It does not self-update, so download a newer
archive when you want one.

## Open something in the next minute

You can also name the folder when you start the app —
`blastradius-app path/to/repo` — and it goes straight there: a workspace
opens, and a folder without one gets the same offer described below.

**Open a folder or repository…** — from the welcome screen or the **Open**
button (`Ctrl+O`) — is the only thing you need. Point it at whatever you have
and it works out what that is:

- a folder with a `blastradius.yaml` opens directly;
- a repository root is searched, and the workspace inside opens — if it holds
  several, you choose;
- a folder with no workspace is **offered one**: a small, commented starter
  model, plus the coding-agent setup that lets you hand the modelling to an
  agent — you choose the pieces (MCP server, skills and instructions) and
  which agents get them, the same choice `blastradius init` offers. You get a
  prompt to paste, too. See [Coding agents (MCP)](agents.md).

You are asked **where** it should go, with `docs/` recommended: a repository
root is for source, and the model is documentation. If your project already
keeps documentation in `docs/` or `doc/`, that one is offered instead of a
near-duplicate. It is only a recommendation — `.` puts the workspace at the
project root.

Nothing you already have is overwritten. Scaffolding into a repository that
already has, say, a `README.md` keeps yours and says so.

There is also **Try a demo workspace** — a throwaway model, useful for a look
around before you point it at anything of your own.

From the CLI, `blastradius init` does the same scaffolding and asks the same
questions, including where to put the workspace (`--into <subdir>` answers it
without prompting, `--agents` the agent setup). `blastradius validate .`
checks a workspace without opening the app, and `blastradius format` prints
the model format in full.

## What you are looking at

The canvas shows **one altitude at a time**, and you fly between them:

| Level | Shows |
| --- | --- |
| **L1** | Systems, and the people and external systems around them |
| **L2** | The containers inside one system |
| **L3** | The components inside one container |
| **L4** | Real modules and types, derived from source |
| **D** | Deployment — where the containers actually run |

Double-click a node to dive into it, press `Esc` to come back up. That is the
whole navigation model. See [Navigating the canvas](canvas.md).

The left panel lists the model as a tree; the right panel inspects whatever is
selected, including the documents attached to it.

## Your own repo

A minimal workspace is two files. `blastradius.yaml` says where things are:

```yaml
workspace:
  name: Acme
  version: 1
model:
  include: [model/*.yaml]
views:
  include: [views/*.yaml]
docs:
  include: ["*.md", "adr/*.md"]
```

And one model file describes a system:

```yaml
system: shop
name: Shop
containers:
  web:
    name: Web
    tech: React
  api:
    name: API
    tech: Go
relations:
  - from: web
    to: api
    label: calls
    protocol: JSON/HTTPS
```

Open the folder and it renders. Everything else — components, deployment,
code-level detail, documents — is additive from here; see the
[model format reference](model-format.md).

## Where things go from here

- Change the model by editing YAML or by dragging on the canvas — both write
  the same files. See [Editing the model](editing.md).
- Commit it. The diff is the point: see
  [Git: diff, history, conflicts](git.md).
- Hand a self-contained HTML file to someone who does not have the app:
  [Sharing and export](export.md).
