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
trap 'rm -f "$actual" "$semantic" "$fallback" "$fallback_err"' EXIT

# ---- 1. syntax mode, byte-exact ---------------------------------------------
echo '{"component":"shop.backend.billing","repoRoot":"fixtures","root":"src","include":[],"exclude":[]}' \
  | dotnet run -c Release > "$actual"

if diff -u fixtures/expected.l4.json "$actual"; then
  echo "csharp extractor: fixture facts byte-exact"
else
  echo "csharp extractor: DRIFT — inspect the diff above; if intentional, refreeze fixtures/expected.l4.json" >&2
  exit 1
fi

# ---- 2. semantic mode resolves what syntax mode cannot ----------------------
# Alpha.Widget and Beta.Widget share a simple name, and Consumer.cs reaches
# Alpha's through a global using in another file: name matching sees two
# candidates and drops the edge, the compiler knows which one it is.
dotnet restore fixtures/semantic/Semantic.sln > /dev/null
echo '{"component":"shop.semantic","repoRoot":"fixtures/semantic","root":".","include":[],"exclude":[],"mode":"semantic"}' \
  | dotnet run -c Release > "$semantic"

if ! grep -q '"blastradius-extract-cs 0.3.0 (semantic)"' "$semantic"; then
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
