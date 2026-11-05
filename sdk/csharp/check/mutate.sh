#!/bin/sh
# Mutation negatives for the C# producer (pcoc#166 r9, stamped shared gate; the maintainer writes it, the builder never edits it).
# Reads check/mutations/manifest.tsv: id <TAB> patch <TAB> expect <TAB> absent1|absent2|... <TAB> pass1|pass2|...
# Pcoc.Check runs with --through <case>: the in-process slice-1 cases (manifest.json, manifest-mutation,
# blockhash-display.json, slot-mutation, slot-mutation-16384, sha256-backend), then producer cases up to
# the named one in AC order, skipping each of rig-O3-PROD, prod-leg and prod-negatives unless it is the
# named case. The full slice-1 set (pack compares, quickstart, negatives canary) and every producer case
# run once, in the honest `dotnet run` of the same CI step.
# Patch rules (checked before anything is applied): every path is an EXISTING regular .cs file under
# sdk/csharp/src/ or sdk/csharp/cli/ (no new, deleted, renamed, copied, mode-changed, symlink or binary
# file; no csproj/props/targets/json/config); every header path agrees with what git applies; no added
# line contains a forbidden token (see deny below). The token list is a speed bump, not containment: the reviewer's
# per-patch review against the named mutation (ticket, reviewer reject list) is the binding control.
# Each row runs in a temp tree holding COPIES of spec/, sdk/SPEC_PIN and sdk/VERSION (read-only) and of
# sdk/csharp (with the honest bin/obj); no link to the real checkout, .git or other SDKs.
# Each row requires: exit 1 exactly; a stderr line starting with the expect line; no absent string on
# stderr; every listed earlier-gate pass line exactly on stdout; no output for a skipped heavy case.
# The rig-O3-PROD row runs first, in the background, in its own process group; each row's dotnet runs in
# its own process group too. On any exit (pass, fail, INT, TERM) the traps kill those groups and remove
# every temp tree, so no dotnet process or tree outlives the script. Build servers, the shared compiler
# (VBCSCompiler) and MSBuild node reuse are off.
set -eu
cd "$(dirname "$0")/.."
if [ -n "${PCOC_ROOT:-}" ]; then root=$PCOC_ROOT; else root=$(CDPATH= cd -- ../.. && pwd); fi
here=$PWD
self="$here/check/mutate.sh"
man=check/mutations/manifest.tsv
want_rows=36
heavy="rig-O3-PROD prod-leg prod-negatives"
export MSBUILDDISABLENODEREUSE=1 DOTNET_CLI_USE_MSBUILD_SERVER=0 UseSharedCompilation=false
child=""
# killgrp pgid: TERM to the whole group (external kill: dash's builtin takes no negative pid).
killgrp() { env kill -s TERM -- "-$1" 2>/dev/null || true; }

clean_tree() { [ -n "${1:-}" ] && [ -d "$1" ] && { chmod -R u+w "$1" 2>/dev/null || true; rm -rf "$1"; }; return 0; }

# check_patch id file: 0 if the patch obeys the patch rules, else prints why on stderr.
check_patch() {
  cid=$1 pf=$2 w=$3
  if ! (cd "$w" && git apply --numstat "$pf") >"$w/numstat" 2>"$w/apply.err"; then
    echo "mutate: $cid: patch does not parse" >&2; cat "$w/apply.err" >&2; return 1
  fi
  [ -s "$w/numstat" ] || { echo "mutate: $cid: patch touches no file" >&2; return 1; }
  bad=0
  while IFS="$tab" read -r add del p; do
    case "$add$del" in *-*) echo "mutate: $cid: binary change: $p" >&2; bad=1; continue ;; esac
    if ! printf '%s\n' "$p" | grep -Eq '^sdk/csharp/(src|cli)/([A-Za-z0-9_.-]+/)*[A-Za-z0-9_-][A-Za-z0-9_.-]*\.cs$' \
       || printf '%s\n' "$p" | grep -Eq '(^|/)\.\.?(/|$)|=>'; then
      echo "mutate: $cid: path not an .cs file under sdk/csharp/src/ or sdk/csharp/cli/: $p" >&2; bad=1; continue
    fi
    if [ ! -f "$root/$p" ] || [ -L "$root/$p" ]; then
      echo "mutate: $cid: not an existing regular file at the tree under test: $p" >&2; bad=1
    fi
  done <"$w/numstat"
  [ "$bad" -eq 0 ] || return 1
  if [ -n "$(cd "$w" && git apply --summary "$pf")" ]; then
    echo "mutate: $cid: patch creates, deletes, renames, copies or changes the mode of a file" >&2
    (cd "$w" && git apply --summary "$pf") >&2; return 1
  fi
  # Every header path must be one of the numstat paths (catches ---/+++ and diff --git name games).
  cut -f3 "$w/numstat" | sort -u >"$w/paths"
  awk '/^diff --git /{ print substr($3, 3); print substr($4, 3); next }
       /^--- /{ p = substr($0, 5); sub(/\t.*/, "", p); sub(/^a\//, "", p); print p; next }
       /^\+\+\+ /{ p = substr($0, 5); sub(/\t.*/, "", p); sub(/^b\//, "", p); print p; next }
       /^(rename|copy) (from|to) /{ print $3 }' "$pf" | sort -u >"$w/hpaths"
  if [ -n "$(comm -13 "$w/paths" "$w/hpaths")" ]; then
    echo "mutate: $cid: header names a path git does not report: $(comm -13 "$w/paths" "$w/hpaths" | tr '\n' ' ')" >&2; return 1
  fi
  if grep -E '^diff --git ' "$pf" | awk '{ a = substr($3, 3); b = substr($4, 3); if (a != b) bad = 1 } END { exit !bad }'; then
    echo "mutate: $cid: diff --git names differ" >&2; return 1
  fi
  # Forbidden tokens on added lines.
  if grep -E '^\+' "$pf" | grep -Ev '^\+\+\+ ' | grep -nE "$deny" >"$w/deny" ; then
    echo "mutate: $cid: added line with a forbidden token:" >&2; cat "$w/deny" >&2; return 1
  fi
  return 0
}
# Speed bump only (a patch can still do harm without these tokens; the reviewer's review is the control):
# The r4 review list (Environment.Exit, Process, Console., File.Write, "fail , "pass ), the r5 review list
# (ModuleInitializer, AssemblyLoadContext, /proc/, Stream, ExitCode, GetType(, GetMethod, Invoke(,
# space-tolerant Environment .), the bare word Environment (catches a member access split across lines),
# Console/File. use, aliases (using static / using X =), \u and \x escapes, native calls (DllImport,
# LibraryImport, extern), Reflection, Activator, and preprocessor lines. r8 (review r6-A, finding R8):
# native interop and function pointers (NativeLibrary, Marshal, unmanaged, unsafe, fixed buffers via
# unsafe), new threads, timers and pools (new Thread, ThreadPool, Timer, Task.Run, Task.Factory),
# delegate declarations (delegate), and AppDomain and GC hooks.
deny='Environment[[:space:]]*\.|(^|[^A-Za-z0-9_])Environment([^A-Za-z0-9_]|$)|ModuleInitializer|AssemblyLoadContext|/proc/|Stream|ExitCode|GetType\(|GetMethod|Invoke\(|Process|Console|File\.|"fail |"pass |using[[:space:]]+static|using[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=|\\u|\\x|DllImport|LibraryImport|extern[[:space:]]|Reflection|Activator|NativeLibrary|Marshal|unmanaged|unsafe|new[[:space:]]+Thread|ThreadPool|Timer|Task\.Run|Task\.Factory|(^|[^A-Za-z0-9_])delegate([^A-Za-z0-9_]|$)|AppDomain|GC\.|^\+[[:space:]]*#'
tab=$(printf '\t')

# run_row id patch expect absent passes: prints one ok line, or a FAIL line on stderr; returns 0/1.
run_row() {
  id=$1 patch=$2 expect=$3 absent=$4 passes=$5
  start=$(date +%s)
  case "$patch" in */*|.*) echo "mutate: $id: patch name '$patch' must be a plain file name in check/mutations" >&2; return 1 ;; esac
  pf="$here/check/mutations/$patch"
  [ -f "$pf" ] && [ ! -L "$pf" ] || { echo "mutate: $id: $patch missing" >&2; return 1; }
  case_name=$(printf '%s\n' "$expect" | sed -n 's/^fail \([^:]*\):.*/\1/p')
  [ -n "$case_name" ] || { echo "mutate: $id: expect is not a 'fail <case>: <why>' line" >&2; return 1; }
  t=$(mktemp -d "${MUTATE_TMP:?}/row.XXXXXX")
  mkdir "$t/sdk"
  cp -R "$root/spec" "$t/spec"
  cp "$root/sdk/SPEC_PIN" "$root/sdk/VERSION" "$t/sdk/"
  chmod -R a-w "$t/spec" "$t/sdk/SPEC_PIN" "$t/sdk/VERSION"
  cp -R "$root/sdk/csharp" "$t/sdk/csharp"
  if ! check_patch "$id" "$pf" "$t"; then
    echo "mutate: $id: patch $patch rejected by the patch rules" >&2; clean_tree "$t"; return 1
  fi
  if ! (cd "$t" && git apply "$pf") 2>"$t/apply.err"; then
    echo "mutate: $id: patch $patch does not apply" >&2; cat "$t/apply.err" >&2; clean_tree "$t"; return 1
  fi
  set +e
  # Own process group (setsid), so a trap can kill this dotnet and everything it starts.
  setsid sh -c 'cd "$1" && PCOC_ROOT= exec dotnet run --disable-build-servers --project check/Pcoc.Check -c Release -- --through "$2"' \
    sh "$t/sdk/csharp" "$case_name" >"$t/out" 2>"$t/err" &
  child=$!
  wait "$child"
  code=$?
  child=""
  set -e
  bad=""
  [ "$code" -eq 1 ] || bad="$bad exit $code (want 1);"
  awk -v e="$expect" 'index($0, e) == 1 { f = 1 } END { exit !f }' "$t/err" || bad="$bad no stderr line starts with '$expect';"
  old=$IFS; IFS='|'
  for a in $absent; do [ -z "$a" ] || [ "$a" = - ] && continue; if grep -qF -- "$a" "$t/err"; then bad="$bad absent '$a' on stderr;"; fi; done
  for p in $passes; do [ -z "$p" ] || [ "$p" = - ] && continue; grep -qxF -- "$p" "$t/out" || bad="$bad earlier gate '$p' missing;"; done
  IFS=$old
  for h in $heavy; do
    [ "$h" = "$case_name" ] && continue
    if grep -qE "^(pass|fail) $h( |:|\$)" "$t/out" "$t/err"; then bad="$bad --through ran $h;"; fi
  done
  if [ -n "$bad" ]; then
    echo "mutate: $id FAIL:$bad" >&2; tail -n 20 "$t/err" >&2; clean_tree "$t"; return 1
  fi
  echo "mutate: $id ok (exit 1, '$expect', $(( $(date +%s) - start )) s)"
  clean_tree "$t"
}

# Internal: the background row re-enters here in its own process group.
if [ "${1:-}" = --bg-row ]; then
  shift
  trap 'if [ -n "$child" ]; then killgrp "$child"; wait "$child" 2>/dev/null || true; fi' EXIT
  trap 'exit 143' TERM; trap 'exit 130' INT
  run_row "$@"; exit $?
fi

[ -s "$man" ] || { echo "mutate: $man missing or empty" >&2; exit 1; }
rows=$(grep -cv '^#' "$man")
[ "$rows" -eq "$want_rows" ] || { echo "mutate: row count $rows (want $want_rows)" >&2; exit 1; }
# Five tab-separated fields per row; an empty list is written as "-" (sh read collapses empty fields).
awk -F '\t' '!/^#/ && (NF != 5 || $1 == "" || $2 == "" || $3 == "" || $4 == "" || $5 == "") { print "mutate: line " NR ": want 5 non-empty fields" > "/dev/stderr"; bad = 1 } END { exit bad }' "$man"
awk -F '\t' '!/^#/ { if (seen[$1]++) { print "mutate: duplicate id " $1 > "/dev/stderr"; bad = 1 } } END { exit bad }' "$man"
res=$(mktemp -d)
bg=""
export MUTATE_TMP="$res"
cleanup() {
  if [ -n "$child" ]; then killgrp "$child"; wait "$child" 2>/dev/null || true; child=""; fi
  if [ -n "$bg" ]; then killgrp "$bg"; wait "$bg" 2>/dev/null || true; bg=""; fi
  clean_tree "$res"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

prod_id=""
grep -v '^#' "$man" > "$res/rows"
while IFS="$tab" read -r id patch expect absent passes; do
  case "$expect" in "fail rig-O3-PROD:"*)
    prod_id=$id
    # setsid: the row and every dotnet it starts share one new process group, so the trap can kill them all.
    setsid sh "$self" --bg-row "$id" "$patch" "$expect" "$absent" "$passes" >"$res/prod.log" 2>&1 &
    bg=$!
    break ;;
  esac
done <"$res/rows"
failed=0
while IFS="$tab" read -r id patch expect absent passes; do
  [ "$id" = "$prod_id" ] && continue
  run_row "$id" "$patch" "$expect" "$absent" "$passes" || { failed=1; break; }
done <"$res/rows"
if [ -n "$bg" ]; then
  if [ "$failed" -eq 1 ]; then killgrp "$bg"; fi
  if wait "$bg"; then prod=0; else prod=1; fi
  bg=""
  cat "$res/prod.log"
  [ "$failed" -eq 1 ] || [ "$prod" -eq 0 ] || failed=1
fi
[ "$failed" -eq 0 ] || exit 1
echo "mutate: $rows mutation negatives ok"
