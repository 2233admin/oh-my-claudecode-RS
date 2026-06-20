#!/usr/bin/env sh
# omc-hud installer — macOS & Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/2233admin/oh-my-claudecode-RS/master/install.sh | sh

set -e

REPO="2233admin/oh-my-claudecode-RS"
BIN="omc-hud"
INSTALL_DIR="$HOME/.local/bin"
SETTINGS="$HOME/.claude/settings.json"

# Detect OS / arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64) ARTIFACT="omc-hud-linux-x86_64" ;;
      *)       echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64)  ARTIFACT="omc-hud-macos-arm64" ;;
      x86_64) ARTIFACT="omc-hud-macos-x86_64" ;;
      *)       echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    echo "Windows users: run install.ps1 in PowerShell"
    exit 1
    ;;
esac

# Get latest release tag
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Error: could not fetch latest release. Check your network or visit:"
  echo "  https://github.com/$REPO/releases"
  exit 1
fi

echo "Installing omc-hud $LATEST..."

URL="https://github.com/$REPO/releases/download/$LATEST/$ARTIFACT"
mkdir -p "$INSTALL_DIR"
curl -fsSL "$URL" -o "$INSTALL_DIR/$BIN"
chmod +x "$INSTALL_DIR/$BIN"

echo "Installed to $INSTALL_DIR/$BIN"

# Ensure ~/.local/bin is in PATH hint
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Hint: add '$INSTALL_DIR' to your PATH (already done in most distros)" ;;
esac

# Wire into Claude Code settings.json
BIN_PATH="$INSTALL_DIR/$BIN"

if [ ! -f "$SETTINGS" ]; then
  mkdir -p "$(dirname "$SETTINGS")"
  cat > "$SETTINGS" << JSON
{
  "statusLine": {
    "type": "command",
    "command": "$BIN_PATH"
  }
}
JSON
  echo "Created $SETTINGS with statusLine config"
else
  # Check if statusLine already set
  if grep -q '"statusLine"' "$SETTINGS" 2>/dev/null; then
    echo "Note: statusLine already set in $SETTINGS — update it manually if needed:"
    echo "  \"command\": \"$BIN_PATH\""
  else
    # Append statusLine using Python (available on macOS/Linux)
    python3 - "$SETTINGS" "$BIN_PATH" << 'PY'
import json, sys
path, bin_path = sys.argv[1], sys.argv[2]
with open(path) as f:
    data = json.load(f)
data["statusLine"] = {"type": "command", "command": bin_path}
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
    echo "Updated $SETTINGS with statusLine config"
  fi
fi

echo ""
echo "Done! Restart Claude Code to see the HUD."
