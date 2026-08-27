#!/usr/bin/env bash
# Fixture gate for the C# extractor (spec/l4-introspection.md). Run from
# anywhere; needs a .NET SDK.
#
# Three checks:
#   1. syntax mode round-trips fixtures/src to byte-exact facts;
#   2. semantic mode resolves an edge syntax mode cannot, on a multi-project
#      solution (the 0.4.0 exit criterion);
#   3. semantic mode falls back to syntax when there is no solution to load.
#
# Plus 2b: semantic dependencies are named by the assembly they live in
# rather than by a using directive's first segment.
#
# Check 2 asserts the resolved edge rather than byte-comparing a frozen file:
# semantic output depends on whichever SDK resolves the symbols, and pinning
# every developer and runner to one SDK buys less than it costs. The exit
# criterion is the edge, not the bytes. Checks 1 and 3 stay byte-exact —
# syntax-level parsing is stable across SDKs.
set -euo pipefail
cd "$(dirname "$0")"

actual="$(mktemp)"
semantic="$(mktemp)"
fallback="$(mktemp)"
fallback_err="$(mktemp)"
absolute="$(mktemp)"
assemblies="$(mktemp)"
trap 'rm -f "$actual" "$semantic" "$fallback" "$fallback_err" "$absolute" "$assemblies"' EXIT

# ---- 1. syntax mode, byte-exact ---------------------------------------------
echo '{"component":"shop.backend.billing","repoRoot":"fixtures","root":"src","include":[],"exclude":[]}' \
  | dotnet run -c Release > "$actual"

if diff -u fixtures/expected.l4.json "$actual"; then
  echo "csharp extractor: fixture facts byte-exact"
else
  echo "csharp extractor: DRIFT — inspect the diff above; if intentional, refreeze fixtures/expected.l4.json" >&2
  exit 1
fi

# ---- 1b. an absolute repoRoot, which is what core actually sends -------------
# This gate only ever passed a *relative* repoRoot, and the dogfood corpus has
# no C# mapping — so nothing exercised the path core really uses, and C#
# introspection was broken on Windows for every absolute root: canonicalize()
# yields the `\\?\C:\...` verbatim form, which takes no separator
# normalization, and joining it with forward slashes is rejected outright
# (found on a packaged install, 2026-08-26).
abs_fixtures="$(cd fixtures && pwd)"
# Under Git Bash, `pwd` is a POSIX path .NET cannot open. `cygpath -m` gives
# the Windows form with forward slashes: valid for .NET and safe inside a JSON
# string, where a backslash would be an escape sequence.
if command -v cygpath > /dev/null 2>&1; then
  abs_fixtures="$(cygpath -m "$abs_fixtures")"
fi
echo "{\"component\":\"shop.backend.billing\",\"repoRoot\":\"$abs_fixtures\",\"root\":\"src\",\"include\":[],\"exclude\":[]}" \
  | dotnet run -c Release > "$absolute"

# Same facts either way: the root is how files are *found*, never part of the
# output — element paths stay repo-relative.
if diff -u fixtures/expected.l4.json "$absolute"; then
  echo "csharp extractor: absolute repoRoot gives the same facts"
else
  echo "csharp extractor: an absolute repoRoot changes the output — see the diff above" >&2
  exit 1
fi

# ---- 2. semantic mode resolves what syntax mode cannot ----------------------
# Alpha.Widget and Beta.Widget share a simple name, and Consumer.cs reaches
# Alpha's through a global using in another file: name matching sees two
# candidates and drops the edge, the compiler knows which one it is.
dotnet restore fixtures/semantic/Semantic.sln > /dev/null
echo '{"component":"shop.semantic","repoRoot":"fixtures/semantic","root":".","include":[],"exclude":[],"mode":"semantic"}' \
  | dotnet run -c Release > "$semantic"

if ! grep -q '"blastradius-extract-cs 0.4.0 (semantic)"' "$semantic"; then
  echo "csharp extractor: semantic mode did not engage — facts say:" >&2
  grep '"extractor"' "$semantic" >&2
  exit 1
fi
if ! python -c "
import json, sys
d = json.load(open(sys.argv[1]))
want = ('Gamma.Consumer', 'Alpha.Widget', 'references')
got = [(e['from'], e['to'], e['kind']) for e in d['edges']]
sys.exit(0 if want in got else print('missing', want, 'in', got) or 1)
" "$semantic"; then
  echo "csharp extractor: semantic mode failed to resolve the cross-project edge" >&2
  exit 1
fi
echo "csharp extractor: semantic mode resolves the cross-project edge"

# ---- 2b. dependencies are named by assembly, not by namespace root ----------
# Map only Beta's files, so Alpha is out of corpus. Syntax mode gets this
# wrong twice over: the global using lives in a file that declares no types,
# so no dependency is recorded at all, and name matching resolves Consumer's
# reference to the in-corpus `Beta.Widget` — the wrong Widget. Semantic mode
# knows the symbol, so it records the real thing and names it by the assembly
# you would add to a project file.
echo '{"component":"shop.semantic","repoRoot":"fixtures/semantic","root":".","include":["Beta/*.cs"],"exclude":[],"mode":"semantic"}'   | dotnet run -c Release > "$assemblies"

if ! python -c "
import json, sys
d = json.load(open(sys.argv[1]))
ids = [e['id'] for e in d['elements']]
edges = [(e['from'], e['to'], e['kind']) for e in d['edges']]
if 'dep.Alpha' not in ids:
    sys.exit(print('no dep.Alpha element; got', ids) or 1)
if ('Gamma.Consumer', 'dep.Alpha', 'imports') not in edges:
    sys.exit(print('dependency not attributed to the referencing type; got', edges) or 1)
if any(e[1] == 'Beta.Widget' for e in edges):
    sys.exit(print('resolved to the wrong Widget:', edges) or 1)
" "$assemblies"; then
  echo "csharp extractor: semantic dependencies are not assembly-named" >&2
  exit 1
fi
echo "csharp extractor: semantic dependencies are named by assembly"

# ---- 3. semantic mode degrades to syntax, never worse -----------------------
# The plain corpus has no solution, so semantic mode must fall back and emit
# exactly what syntax mode emits, differing only in the recorded mode.
echo '{"component":"shop.backend.billing","repoRoot":"fixtures","root":"src","include":[],"exclude":[],"mode":"semantic"}' \
  | dotnet run -c Release > "$fallback" 2> "$fallback_err"

if ! grep -q "falling back to syntax-level" "$fallback_err"; then
  echo "csharp extractor: expected a fallback warning on stderr, got:" >&2
  cat "$fallback_err" >&2
  exit 1
fi
if ! diff -q \
  <(sed 's/(syntax-fallback)/(syntax)/' "$fallback") \
  "$actual" > /dev/null; then
  echo "csharp extractor: fallback output differs from syntax mode — it must never be worse" >&2
  diff -u "$actual" "$fallback" >&2 || true
  exit 1
fi
echo "csharp extractor: semantic mode falls back cleanly to syntax"
