// Stage the out-of-process extractors beside a binary.
//
// Both installers need this and neither may drift from the other: core looks
// for `current_exe()/../extractors` first, so an installed layout without that
// directory can introspect nothing but Rust (which is built into core). The
// MSIX shipped without it from 0.1.0 through 0.5.0 — every published Store
// build silently failed TypeScript and C# introspection until the first
// outside user hit it (docs/roadmap.md, first-user findings).
//
// The C# extractor is *published*, not copied as source. An installed package
// directory is read-only — under MSIX emphatically so — and `dotnet run
// --project` wants to write bin/ and obj/ next to the project, so shipping the
// csproj would have failed a second time on an installed machine. Publishing
// also drops the requirement from a .NET SDK plus a first-run NuGet restore
// (itself a failure mode on a locked-down desktop) to a .NET runtime alone;
// only opt-in semantic mode still needs an SDK, for the target solution.
//
// Used as a module by tools/stage-portable.mjs, and from the command line by
// tools/pack-msix.ps1:
//
//   node tools/stage-extractors.mjs --out packaging/msix/dist

import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, realpathSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Build output and test data — none of it belongs in a shipped package. */
const EXCLUDE = /[\\/](node_modules|bin|obj|fixtures)([\\/]|$)/;

/** What core looks for to decide the C# extractor is a published build. */
export const CSHARP_ENTRY = 'BlastradiusExtract.dll';

/**
 * Copy/publish the extractor trees under `out`, preserving the `extractors/`
 * prefix. Returns the staged directories.
 */
export function stageExtractors(out) {
  // TypeScript: a single dependency-free script. Nothing to build, and it
  // runs fine from a read-only directory.
  cpSync(join(root, 'extractors/typescript'), join(out, 'extractors/typescript'), {
    recursive: true,
    filter: (src) => !EXCLUDE.test(src),
  });

  // Publish into a fixed spot and copy from there. `-o` with an arbitrary
  // absolute path is not reliable — SDK 10 hands some of them to MSBuild as a
  // bare `PublishDir=...` positional, which comes back as "project file does
  // not exist" — and one relative path we control avoids the whole question.
  const staging = join(root, 'target/extractor-publish');
  const project = join(root, 'extractors/dotnet');
  const res = spawnSync(
    'dotnet',
    ['publish', 'BlastradiusExtract.csproj', '-c', 'Release', '-o', '../../target/extractor-publish', '--nologo'],
    { cwd: project, stdio: 'inherit', shell: process.platform === 'win32' }
  );
  if (res.error?.code === 'ENOENT') {
    throw new Error('dotnet SDK not found — needed to publish the C# extractor');
  }
  if (res.status !== 0) throw new Error(`dotnet publish failed (${res.status})`);
  if (!existsSync(join(staging, CSHARP_ENTRY))) {
    throw new Error(`dotnet publish produced no ${CSHARP_ENTRY} in ${staging}`);
  }

  const dotnet = join(out, 'extractors/dotnet');
  cpSync(staging, dotnet, { recursive: true });

  return [join(out, 'extractors/typescript'), dotnet];
}

const invokedDirectly =
  process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href;

if (invokedDirectly) {
  const i = process.argv.indexOf('--out');
  if (i < 0 || !process.argv[i + 1]) throw new Error('missing --out <dir>');
  const out = resolve(root, process.argv[i + 1]);
  for (const dir of stageExtractors(out)) console.log(`staged ${dir}`);
}
