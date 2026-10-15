#!/usr/bin/env bash
# Path gate for ci.yml. Usage: changes.sh <base-sha|""> <head-sha>
# Prints key=value lines (spec rust go java gate claims bari index desktop docs).
# Empty, all-zero or unknown base => run everything. spec/ or .github/ or any
# unclassified path => run everything. spec/* already covers spec/conformance/.
# A file reintroduced at repo-root conformance/ hits the unclassified arm.
# A suite whose directory is absent is always false.
set -euo pipefail
base="${1:-}"; head="${2:-HEAD}"
keys=(spec rust go java gate claims bari index desktop docs)
declare -A on dir=([spec]=spec [rust]=reference/rust [go]=reference/go [java]=reference/java
  [gate]=services/admission-gate [claims]=services/admission-claims [bari]=services/bari [index]=services/index
  [desktop]=apps/desktop [docs]=.)
for k in "${keys[@]}"; do on[$k]=false; done
all=false
if [[ -z "$base" || "$base" =~ ^0+$ ]] || ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  all=true; echo "base unknown: full run" >&2
else
  mapfile -t files < <(git diff --name-only "$base" "$head")
  for f in "${files[@]}"; do
    case "$f" in
      spec/*|.github/*) all=true ;;
      reference/rust/*) on[rust]=true ;;
      reference/go/*) on[go]=true ;;
      reference/java/*) on[java]=true ;;
      reference/*) on[rust]=true; on[go]=true; on[java]=true ;;
      services/admission-gate/*) on[gate]=true ;;
      services/admission-claims/*) on[claims]=true ;;
      services/bari/*) on[bari]=true ;;
      services/index/*) on[index]=true ;;
      services/*.md|apps/*.md|docs/*|*.md|LICENSE) on[docs]=true ;;
      apps/desktop/*) on[desktop]=true ;;
      .gitignore|.gitleaks.toml) ;;  # scans always run
      *) all=true; echo "unclassified: $f -> full run" >&2 ;;
    esac
  done
fi
for k in "${keys[@]}"; do
  v=${on[$k]}; [[ $all == true ]] && v=true
  [[ -d "${dir[$k]}" ]] || v=false
  echo "$k=$v"
done
