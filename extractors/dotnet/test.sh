#!/usr/bin/env bash
# Fixture gate for the C# extractor (spec/l4-introspection.md): the corpus in
# fixtures/src must round-trip to byte-exact facts. Run from anywhere; needs
# a .NET SDK (the extractor itself needs no restore of any target solution).
set -euo pipefail
cd "$(dirname "$0")"

actual="$(mktemp)"
trap 'rm -f "$actual"' EXIT

echo '{"component":"shop.backend.billing","repoRoot":"fixtures","root":"src","include":[],"exclude":[]}' \
  | dotnet run -c Release > "$actual"

if diff -u fixtures/expected.l4.json "$actual"; then
  echo "csharp extractor: fixture facts byte-exact"
else
  echo "csharp extractor: DRIFT — inspect the diff above; if intentional, refreeze fixtures/expected.l4.json" >&2
  exit 1
fi
