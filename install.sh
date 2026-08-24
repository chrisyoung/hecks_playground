#!/usr/bin/env bash
set -e

HECKS_PLAYGROUND_HOME="${HECKS_PLAYGROUND_HOME:-$HOME/.hecks_playground}"
REPO="https://github.com/chrisyoung/hecks_playground.git"

echo "Installing HecksPlayground..."

# Clone or update
if [ -d "$HECKS_PLAYGROUND_HOME" ]; then
  echo "  Updating $HECKS_PLAYGROUND_HOME..."
  git -C "$HECKS_PLAYGROUND_HOME" pull --ff-only 2>/dev/null || git -C "$HECKS_PLAYGROUND_HOME" fetch && git -C "$HECKS_PLAYGROUND_HOME" reset --hard origin/main
else
  echo "  Cloning to $HECKS_PLAYGROUND_HOME..."
  git clone "$REPO" "$HECKS_PLAYGROUND_HOME"
fi

# Bundle
echo "  Installing dependencies..."
cd "$HECKS_PLAYGROUND_HOME"
bundle install --quiet 2>/dev/null || echo "  (bundle install skipped — run manually if needed)"

# Symlink
BIN_DIR="/usr/local/bin"
if [ ! -w "$BIN_DIR" ]; then
  BIN_DIR="$HOME/.local/bin"
  mkdir -p "$BIN_DIR"
fi

ln -sf "$HECKS_PLAYGROUND_HOME/bin/hecks_playground" "$BIN_DIR/hecks_playground"
chmod +x "$HECKS_PLAYGROUND_HOME/bin/hecks_playground"

# Make sure bin/hecks_playground always resolves back to HECKS_PLAYGROUND_HOME
export HECKS_PLAYGROUND_HOME

echo ""
echo "HecksPlayground installed!"
echo "  Location: $HECKS_PLAYGROUND_HOME"
echo "  Binary:   $BIN_DIR/hecks_playground"
echo ""

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -q "^$BIN_DIR$"; then
  echo "Add to your shell profile:"
  echo "  export PATH=\"$BIN_DIR:\$PATH\""
fi
