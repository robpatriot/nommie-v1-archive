#!/usr/bin/env bash
set -euo pipefail

# Config
HOOK_PATH=".githooks/pre-commit"
RUST_DUMMY="apps/backend/src/dummy_precommit.rs"
TS_DUMMY="apps/frontend/dummy_precommit.ts"
JSON_DUMMY="apps/frontend/dummy_precommit.json"

# Helpers
repo_root() { git rev-parse --show-toplevel; }
hr() { printf '%*s\n' "${COLUMNS:-80}" '' | tr ' ' -; }
status() { echo; echo "→ git status (short):"; git status --porcelain; echo; }

ensure_clean_start() {
  if ! git diff --quiet --cached; then
    echo "⚠️  You currently have STAGED changes."
    echo "    This test only manipulates dummy files, but for a clean run:"
    echo "    - Unstage with: git restore --staged <paths>"
    echo "    - Or continue anyway by setting ALLOW_DIRTY=1"
    [[ "${ALLOW_DIRTY:-0}" == "1" ]] || { echo "Aborting."; exit 1; }
  fi
}

ensure_tools() {
  command -v pnpm >/dev/null || { echo "pnpm not found in PATH"; exit 1; }
  [[ -x "$HOOK_PATH" ]] || { echo "Hook not found or not executable: $HOOK_PATH"; exit 1; }
}

cleanup_file() {
  local f="$1"
  # Unstage if staged (modern syntax); ignore errors if not staged
  git restore --staged "$f" 2>/dev/null || true
  # Restore working copy from HEAD; deletes the file if it didn't exist in HEAD
  git restore "$f" 2>/dev/null || true
}

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
  "$HOOK_PATH" || { echo "❌ Hook failed during: $title"; status; exit 1; }

  echo "[post-run] show staged changes for $file:"
  git diff --cached -- "$file" || true

  echo "[cleanup] reverting $file and unstaging"
  cleanup_file "$file"

  status
  echo "✅ $title OK"
}

main() {
  cd "$(repo_root)"
  ensure_tools
  ensure_clean_start

  echo "=== Pre-commit hook test runner ==="
  echo "Repo: $(pwd)"
  echo "Hook: $HOOK_PATH"
  echo

  # Case 1: Rust (clippy + rustfmt)
  run_case "Rust branch (clippy + rustfmt)" "$RUST_DUMMY" 'fn main(){println!("hi");}'

  # Case 2: ESLint + Prettier on TS
  run_case "ESLint + Prettier on .ts" "$TS_DUMMY" 'const foo:any=42;console.log(foo)'

  # Case 3: Prettier-only on JSON
  run_case "Prettier-only on .json" "$JSON_DUMMY" '{"foo":   "bar"}'

  # Case 4: No staged files
  echo
  hr
  echo "🧪 No staged files case"
  hr
  status
  echo "[run] executing $HOOK_PATH with nothing staged"
  "$HOOK_PATH" || { echo "❌ Hook failed in no-staged case"; status; exit 1; }
  echo "✅ No staged files case OK"

  echo
  echo "🎉 All tests completed successfully."
}

main "$@"

