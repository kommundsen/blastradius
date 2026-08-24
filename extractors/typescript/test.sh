#!/usr/bin/env bash
# Fixture gate for the TypeScript extractor (spec/l4-introspection.md): the
# corpus in fixtures/src must round-trip to byte-exact facts. Run from
# anywhere; needs Node and the repo's `typescript` dependency (npm ci).
set -euo pipefail
cd "$(dirname "$0")"

actual="$(mktemp)"
trap 'rm -f "$actual"' EXIT

echo '{"component":"shop.web.ui","repoRoot":"fixtures","root":"src","include":[],"exclude":[]}' \
  | node extract.mjs > "$actual"

if diff -u fixtures/expected.l4.json "$actual"; then
  echo "typescript extractor: fixture facts byte-exact"
else
  echo "typescript extractor: DRIFT — inspect the diff above; if intentional, refreeze fixtures/expected.l4.json" >&2
  exit 1
fi
