#!/bin/sh
# Build canbec from this checkout and install it to /Applications: tools/install-macos.sh
# Run it again after `git pull` to update a source install. Release installs can update
# themselves from Settings (see infrastructure/src/updater.rs).
set -e
cd "$(dirname "$0")/.."
cargo build --release -p canbec
tools/bundle-macos.sh
codesign --force --deep --sign - dist/canbec.app # ad-hoc, like cargo's own signature, now for the bundle
pkill -x canbec 2>/dev/null && sleep 1 || true
rm -rf /Applications/canbec.app
ditto dist/canbec.app /Applications/canbec.app
echo "installed /Applications/canbec.app"
