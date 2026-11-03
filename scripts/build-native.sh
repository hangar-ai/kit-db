#!/bin/sh
# Builds the Database tab (native/) to WebAssembly and puts the module where hangar-kit.json's
# "native" says: tabs/db.wasm.
#
#   scripts/build-native.sh           build it, and write tabs/db.wasm
#   scripts/build-native.sh --check   build it, and fail if tabs/db.wasm is not what the source builds
#
# Hangar runs the .wasm a kit ships and never builds one, so commit the module in the same commit as
# the source that changes it. Run from anywhere. Needs rustup: rust-toolchain.toml names the compiler
# and the wasm32-unknown-unknown target, which rustup fetches when they are missing.
set -eu

cd "$(dirname "$0")/.."
kit="$(pwd)"
module=tabs/db.wasm

if command -v rustup >/dev/null 2>&1; then
  rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install
fi

# The same source must give the same bytes in any folder, for any user, or --check could not tell a
# stale module from a fresh one. A panic message holds the path of the file it comes from: these keep
# the names of your folders (your user name, with them) out of a file you publish. Fields are
# separated by \037, so a folder with a space in its name is fine.
sysroot="$(rustc --print sysroot)"
CARGO_ENCODED_RUSTFLAGS="$(printf '%s\037%s\037%s' \
  "--remap-path-prefix=$kit=/kit" \
  "--remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo" \
  "--remap-path-prefix=$sysroot=/rust")"
export CARGO_ENCODED_RUSTFLAGS

cargo build --manifest-path native/Cargo.toml --release --locked --target wasm32-unknown-unknown
built=native/target/wasm32-unknown-unknown/release/kit_db_tab.wasm

if [ "${1:-}" = "--check" ]; then
  if cmp -s "$built" "$module"; then
    echo "$module is what native/ builds."
  else
    echo "$module is stale: run scripts/build-native.sh and commit it." >&2
    exit 1
  fi
else
  cp "$built" "$module"
  chmod 644 "$module"
  echo "$module ($(wc -c <"$module") bytes)"
fi
