#!/usr/bin/env bash
set -euo pipefail

# Stores a newly issued test key outside the repository with owner-only access.
# Run this yourself in Terminal; the key is never echoed or written to history.

CONFIG_DIR="${FORGECAD_LOCAL_CONFIG_DIR:-$HOME/Library/Application Support/ForgeCAD}"
KEY_FILE="${FORGECAD_DEEPSEEK_API_KEY_FILE:-$CONFIG_DIR/deepseek-test.key}"

umask 077
mkdir -p "$(dirname "$KEY_FILE")"
read -r -s "DEEPSEEK_TEST_KEY?Paste a newly issued DeepSeek test key: "
printf '\n'
if [[ -z "$DEEPSEEK_TEST_KEY" ]]; then
  echo "No key was entered; nothing was saved." >&2
  exit 2
fi
printf '%s' "$DEEPSEEK_TEST_KEY" > "$KEY_FILE"
unset DEEPSEEK_TEST_KEY
chmod 600 "$KEY_FILE"

echo "Stored a local test credential at $KEY_FILE"
echo "Run ./script/build_and_run.sh, then start the local Agent from ForgeCAD Settings."
