// Stage a portable, install-free bundle (docs/roadmap.md 0.5.0 theme 3).
//
// Windows distribution is otherwise Store-only, which leaves nothing for a
// machine with the Store removed — a locked-down or Intune-managed desktop, an
// air-gapped build box. The MSIX that CI produces cannot fill the gap either:
// it is deliberately unsigned because the Store signs during ingestion, so it
// will not sideload.
//
// Usage: node tools/stage-portable.mjs --out dist/<name> --version 0.5.0 --target windows-x64

import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && process.argv[i + 1]) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing --${name}`);
};

const out = join(root, arg('out'));
const version = arg('version');
const target = arg('target');
const windows = target.startsWith('windows');
const exe = (name) => (windows ? `${name}.exe` : name);

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

// The two binaries. blastradius-app carries the whole UI compiled in
// (tauri.conf.json frontendDist), so there is nothing else to copy for it.
for (const bin of ['blastradius', 'blastradius-app']) {
  const from = join(root, 'target', 'release', exe(bin));
  if (!existsSync(from)) throw new Error(`missing build output: ${from}`);
  cpSync(from, join(out, exe(bin)));
}

// The out-of-process extractors. Core looks for them beside the running
// binary first (`current_exe()/../extractors`), which is precisely the
// installed layout this bundle is — without them, TypeScript and C#
// introspection would fail on a machine that has no checkout. Rust is built
// into core and needs nothing.
for (const rel of ['extractors/typescript', 'extractors/dotnet']) {
  cpSync(join(root, rel), join(out, rel), {
    recursive: true,
    filter: (src) => !/[\\/](node_modules|bin|obj|fixtures)([\\/]|$)/.test(src),
  });
}

cpSync(join(root, 'LICENSE'), join(out, 'LICENSE'));

const runtime = windows
  ? 'Windows 10 1809 or newer. Nothing to install: unzip and run.'
  : [
      'A glibc Linux with WebKitGTK available — on Debian/Ubuntu:',
      '',
      '    sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0',
      '',
      'The CLI (blastradius) needs none of that and runs on its own.',
    ].join('\n');

writeFileSync(
  join(out, 'README.txt'),
  `Blastradius ${version} — portable (${target})

Two programs, no installer, no admin rights, nothing written outside the
folder you run them from:

  ${exe('blastradius-app')}   the desktop app
  ${exe('blastradius')}       the command line (validate, introspect, export, mcp)

${runtime}

Getting started
---------------
Run ${exe('blastradius-app')} and open a folder containing a blastradius.yaml —
or your repository root, and the workspace inside is found for you. Press ?
in the app for the full help, which ships inside it and needs no network.

From the command line:

    ${exe('blastradius')} init .          scaffold a starter workspace
    ${exe('blastradius')} validate .      check a workspace
    ${exe('blastradius')} export . -o architecture.html

The extractors/ folder next to these binaries is what lets code-level (L4)
introspection work for TypeScript and C#; leave it where it is. TypeScript
additionally needs Node, and C# needs a .NET SDK — only if you use them.

Updates
-------
This build does not update itself. On Windows the Microsoft Store edition
does, and is the better choice where the Store is available. Otherwise
download a newer archive when you want one:

    https://github.com/kommundsen/blastradius/releases

Everything the app does happens on your machine; it has no server and sends
nothing anywhere. See the Privacy page in the in-app help.
`
);

console.log(`staged ${target} bundle in ${out}`);
