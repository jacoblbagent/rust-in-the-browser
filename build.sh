#!/usr/bin/env bash
# Build the wasm package into www/pkg. User-local toolchain, no root required.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
cd "$(dirname "$0")"

# simd128 lets LLVM use the four-wide wasm vector instructions. JavaScript has
# no equivalent, which is where the head-to-head benchmark gets its margin.
export RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+simd128"

wasm-pack build --target web --release --out-dir www/pkg "$@"

# wasm-pack drops an ignore-everything .gitignore into the out dir. The built
# module is part of the deliverable, so keep it tracked.
rm -f www/pkg/.gitignore

echo "=== payload ==="
ls -l www/pkg/*.wasm | awk '{printf "%s  %.0f KB\n", $9, $5/1024}'
