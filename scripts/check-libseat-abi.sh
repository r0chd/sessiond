#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$script_dir/.."

upstream=${1:-"$(pkg-config --variable=libdir libseat)/libseat.so"}
ours=${2:-target/debug/libseat.so}

if [[ $# -lt 2 ]]; then
	cargo build -p seat
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf 'upstream: %s\n' "$upstream"
printf 'ours:     %s\n' "$ours"

nm -D --defined-only "$upstream" | awk '{print $3}' | grep '^libseat_' | sort > "$tmp/upstream.symbols"
nm -D --defined-only "$ours" | awk '{print $3}' | grep '^libseat_' | sort > "$tmp/ours.symbols"

printf '\n== libseat exported symbols ==\n'
diff -u "$tmp/upstream.symbols" "$tmp/ours.symbols"

upstream_soname=$(readelf -d "$upstream" | sed -n 's/.*Library soname: \[\(.*\)\].*/\1/p')
ours_soname=$(readelf -d "$ours" | sed -n 's/.*Library soname: \[\(.*\)\].*/\1/p')

printf '\n== SONAME ==\n'
printf 'upstream: %s\n' "${upstream_soname:-<none>}"
printf 'ours:     %s\n' "${ours_soname:-<none>}"

printf '\n== abidiff ==\n'
abidiff "$upstream" "$ours"
