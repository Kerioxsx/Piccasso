#!/bin/sh
# Adds Piccasso to your app menu (no root needed). Run from the unpacked folder.
set -e
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
cp "$here/piccasso" "$HOME/.local/bin/piccasso"
cp "$here/piccasso.png" "$HOME/.local/share/icons/hicolor/512x512/apps/piccasso.png"
sed "s|^Exec=.*|Exec=$HOME/.local/bin/piccasso|" "$here/piccasso.desktop" > "$HOME/.local/share/applications/piccasso.desktop"
echo "Piccasso is in your app menu. You can also run: $HOME/.local/bin/piccasso"
