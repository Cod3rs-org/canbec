#!/bin/sh
# Screenshot canbec on a URL with a throwaway profile (macOS):
#   tools/shoot.sh <url> <out.png> [state.json] [wait-seconds]
set -e
dir=$(mktemp -d)
[ -n "$3" ] && cp "$3" "$dir/state.json"
CANBEC_CONFIG_DIR="$dir" ./target/release/canbec "$1" & pid=$!
sleep "${4:-8}"
swift tools/winshot.swift canbec "$2"
kill $pid; rm -rf "$dir"
