#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"
protocol=${FAKE_INPUT_XML:-/usr/share/plasma-wayland-protocols/fake-input.xml}
test -f "$protocol" || { echo 'Set FAKE_INPUT_XML to Plasma fake-input.xml' >&2; exit 1; }
mkdir -p target/integration
wayland-scanner client-header "$protocol" target/integration/fake-input-client.h
wayland-scanner private-code "$protocol" target/integration/fake-input-protocol.c
read -r -a wayland_flags <<< "$(pkg-config --cflags --libs wayland-client)"
"${CC:-cc}" -I target/integration tests/integration/input.c target/integration/fake-input-protocol.c \
    "${wayland_flags[@]}" -lm -o target/integration/input
