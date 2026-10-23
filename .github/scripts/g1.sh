#!/usr/bin/env bash
# G1 leak grep. The pattern is the G1 regex from the release plan. It is read from $G1_PATTERNS
# (a repository secret) so the names it looks for never land in this public tree.
# A hit is ignored only when its exact "path:line" text is listed in .github/g1-allowlist.txt
# (reviewed false positives; one whole line each, so any edit to the line makes it a hit again).
# Exits 1 on any other hit, or if the pattern is missing. Run from the repository root.
set -uo pipefail
if [[ -z "${G1_PATTERNS:-}" ]]; then
  echo "::error::G1_PATTERNS is not set (repository secret); refusing to pass G1 without it"; exit 1
fi
ALLOW=.github/g1-allowlist.txt
rc=0
hits=$(rg -n -i -e "$G1_PATTERNS" --glob '!**/vectors-allowlist/**' --glob '!.github/g1-allowlist.txt' --hidden --glob '!.git/**' . || true)
if [[ -n "$hits" ]]; then
  left=$(printf '%s\n' "$hits" | python3 .github/scripts/g1_filter.py "$ALLOW")
  if [[ -n "$left" ]]; then
    printf '%s\n' "$left"; echo "::error::G1 leak grep has hits (above)"; rc=1
  fi
fi
if rg -n -i 'grounding ?lab' --hidden --glob '!.git/**' . | rg -v groundinglab; then
  echo "::error::name spelling: use lowercase groundinglab"; rc=1
fi
[[ $rc == 0 ]] && echo "G1 = 0"
exit $rc
