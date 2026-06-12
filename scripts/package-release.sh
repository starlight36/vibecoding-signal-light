#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <version> <platform> [binary] [out-dir]" >&2
  echo "Example: $0 v0.1.0 macos-aarch64 native/target/release/signal-light-native dist" >&2
}

if [[ $# -lt 2 || $# -gt 4 ]]; then
  usage
  exit 2
fi

VERSION="$1"
PLATFORM="$2"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${3:-$ROOT_DIR/native/target/release/signal-light-native}"
OUT_DIR="${4:-$ROOT_DIR/dist}"

if [[ ! -x "$BINARY" ]]; then
  echo "Release binary is missing or not executable: $BINARY" >&2
  exit 1
fi

PACKAGE_NAME="signal-light-${VERSION}-${PLATFORM}"
WORK_DIR="$OUT_DIR/$PACKAGE_NAME"
ARCHIVE="$OUT_DIR/${PACKAGE_NAME}.tar.gz"

rm -rf "$WORK_DIR" "$ARCHIVE"
mkdir -p "$WORK_DIR/bin" "$WORK_DIR/scripts" "$WORK_DIR/docs/images" "$OUT_DIR"

install -m 755 "$BINARY" "$WORK_DIR/bin/signal-light-native"
for script in signal-light install-hooks codex-signal-hook claude-code-signal-hook; do
  install -m 755 "$ROOT_DIR/scripts/$script" "$WORK_DIR/scripts/$script"
done

install -m 644 "$ROOT_DIR/README.md" "$WORK_DIR/README.md"
install -m 644 "$ROOT_DIR/LICENSE" "$WORK_DIR/LICENSE"
install -m 644 "$ROOT_DIR/docs/LAMP_LANGUAGE.md" "$WORK_DIR/docs/LAMP_LANGUAGE.md"
if [[ -f "$ROOT_DIR/docs/images/demo.jpg" ]]; then
  install -m 644 "$ROOT_DIR/docs/images/demo.jpg" "$WORK_DIR/docs/images/demo.jpg"
fi

cat > "$WORK_DIR/RELEASE.txt" <<EOF
Signal Light $VERSION
Platform: $PLATFORM

Quick smoke test:
  ./scripts/signal-light --help
  ./scripts/signal-light play working --dry-run

The wrapper scripts in this archive use ./bin/signal-light-native automatically.
EOF

(
  cd "$OUT_DIR"
  COPYFILE_DISABLE=1 tar -czf "$(basename "$ARCHIVE")" "$PACKAGE_NAME"
)

rm -rf "$WORK_DIR"
printf '%s\n' "$ARCHIVE"
