#!/usr/bin/env bash
set -euo pipefail

# Config
HOOK_PATH=".githooks/pre-commit"
RUST_DUMMY="apps/backend/src/dummy_precommit.rs"
TS_DUMMY="apps/frontend/dummy_precommit.ts"
JSON_DUMMY="apps/frontend/dummy_precommit.json"

# Track everything we create so we can always clean up on exit
DUMMIES=("$RUST_DUMMY" "$TS_DUMMY" "$JSON_DUMMY")

# Helpers
repo_root() { git rev-parse --show-toplevel; }
hr() { printf '%*s\n' "${COLUMNS:-80}" '' | tr ' ' -; }
status() { echo; echo "→ git status (short):"; git status --porcelain; echo; }

ensure_clean_start() {
  if ! git diff --quiet --cached && [[ "${ALLOW_DIRTY:-0}" != "1" ]]; then
    echo "⚠️  You currently have STAGED changes."
    echo "    Unstage with: git restore --staged ."
    echo "    Or allow dirty run with: ALLOW_DIRTY=1 $0"
    exit 1
  fi
}

ensure_tools() {
  command -v pnpm >/dev/null || { echo "pnpm not found in PATH"; exit 1; }
  [[ -x "$HOOK_PATH" ]] || { echo "Hook not found or not executable: $HOOK_PATH"; exit 1; }
}

is_tracked() {
  git ls-files --error-unmatch "$1" >/dev/null 2>&1
}

cleanup_file() {
  local f="$1"
  # Always unstage if staged (no-op if not staged)
  git restore --staged "$f" 2>/dev/null || true

  if is_tracked "$f"; then
    # Tracked: restore content from HEAD (reverts edits or re-creates if deleted)
    git restore "$f" 2>/dev/null || true
  else
    # Untracked: delete precisely this path via Git (safe, scoped)
    git clean -f -- "$f" 2>/dev/null || true
  fi
}

cleanup_all() {
  for f in "${DUMMIES[@]}"; do
    cleanup_file "$f"
  done
}
trap cleanup_all EXIT

run_case() {
  local title="$1" file="$2" contents="$3"
  echo
  hr
  echo "🔎 $title"
  hr

  echo "[setup] writing $file"
  mkdir -p "$(dirname "$file")"
  printf "%s\n" "$contents" > "$file"

  echo "[setup] staging $file"
  git add "$file"

  status

  echo "[run] executing $HOOK_PATH"
  set +e
  "$HOOK_PATH"
  local rc=$?
  set -e

  echo "[post-run] staged diff for $file (if any):"
  git diff --cached -- "$file" || true

  echo "[cleanup] reverting $file and unstaging"
  cleanup_file "$file"
  status

  if [[ $rc -ne 0 ]]; then
    echo "❌ Hook failed during: $title (exit $rc)"
    if [[ "${CONTINUE_ON_FAIL:-0}" == "1" ]]; then
      echo "↪️  Continuing to next case (CONTINUE_ON_FAIL=1)"
    else
      echo "💡 Tip: set CONTINUE_ON_FAIL=1 to run remaining cases even if one fails."
      exit $rc
    fi
  else
    echo "✅ $title OK"
  fi
}

main() {
  cd "$(repo_root)"
  ensure_tools
  ensure_clean_start

  echo "=== Pre-commit hook test runner ==="
  echo "Repo: $(pwd)"
  echo "Hook: $HOOK_PATH"
  echo "Options: ALLOW_DIRTY=${ALLOW_DIRTY:-0} CONTINUE_ON_FAIL=${CONTINUE_ON_FAIL:-0}"
  echo

  # Case 1: Rust (clippy + rustfmt) – should PASS
  run_case "Rust branch (clippy + rustfmt)" "$RUST_DUMMY" 'fn main(){println!("hi");}'

  # Case 2: ESLint + Prettier on TS – intentionally FAILS (no-explicit-any)
  run_case "ESLint + Prettier on .ts" "$TS_DUMMY" 'const foo:any=42;console.log(foo)'

  # Case 3: Prettier-only on JSON – should PASS
  run_case "Prettier-only on .json" "$JSON_DUMMY" '{"foo":   "bar"}'

  # Case 4: No staged files – should PASS
  echo
  hr
  echo "🧪 No staged files case"
  hr
  status
  echo "[run] executing $HOOK_PATH with nothing staged"
  set +e
  "$HOOK_PATH"
  local rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    echo "❌ Hook failed in no-staged case (exit $rc)"; exit $rc
  fi
  echo "✅ No staged files case OK"

  echo
  echo "🎉 All tests completed."
}

main "$@"

