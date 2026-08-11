#!/bin/sh
set -eu

target="${1:?usage: scripts/package-macos-app.sh <target-triple>}"
case "$target" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *)
        echo "unsupported macOS target: $target" >&2
        exit 2
        ;;
esac

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
test -n "$version"

cargo bundle --release --target "$target" --format osx --bin token

app_path="target/$target/release/bundle/osx/Token.app"
artifact_dir="target/distrib"
artifact_path="$artifact_dir/Token-$version-$target.app.tar.gz"
test -d "$app_path"
mkdir -p "$artifact_dir"
COPYFILE_DISABLE=1 tar -C "$(dirname "$app_path")" -czf "$artifact_path" "$(basename "$app_path")"
echo "$artifact_path"
