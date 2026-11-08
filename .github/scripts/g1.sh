#!/usr/bin/env bash
# G1. Two checks, run from the repository root:
# 1. Leak grep (blocking). The pattern is the G1 regex from the release plan. It is read from
#    $G1_PATTERNS (a repository secret) so the names it looks for never land in this public tree.
#    A hit is ignored only when its exact "path:line" text is listed in .github/g1-allowlist.txt
#    (reviewed false positives; one whole line each, so any edit to the line makes it a hit again).
#    Exits 1 on any other hit, or if the pattern is missing.
# 2. Name spelling (advisory). The public name is lowercase "groundinglab". A wrong spelling
#    emits ::warning and does not change the exit code. It is not a secret and not a SPEC term.
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
# Editorial only. Do not set rc.
spelling=$(rg -n -i --color=never 'grounding ?lab' --hidden --glob '!.git/**' . | rg -v --color=never groundinglab || true)
if [[ -n "$spelling" ]]; then
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    echo "name spelling (advisory): ${line}"
    if [[ "$line" =~ ^(\./)?([^:]+):([0-9]+): ]]; then
      echo "::warning file=${BASH_REMATCH[2]},line=${BASH_REMATCH[3]}::name spelling: use lowercase groundinglab"
    else
      echo "::warning::name spelling: use lowercase groundinglab"
    fi
  done <<< "$spelling"
fi
[[ $rc == 0 ]] && echo "G1 = 0"
exit $rc
