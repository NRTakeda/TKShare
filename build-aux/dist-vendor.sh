#!/bin/sh
# Since Meson invokes this script as
# "/bin/sh .../dist-vendor.sh DIST SOURCE_ROOT" we can't rely on bash features
set -eu
export DIST="$1"
export SOURCE_ROOT="$2"

cd "$SOURCE_ROOT"
mkdir "$DIST"/.cargo

# NOTE: this project vendors the protocol engine source under "vendor/rquickshare"
# (versioned in git, referenced by a [patch] path in Cargo.toml). So we must NOT
# write the registry crates into "vendor/" — that would clobber the engine and
# break the move below. Generate them into a separate "vendor-crates" dir.
VENDOR_CRATES="vendor-crates"
rm -rf "$VENDOR_CRATES"

# cargo-vendor-filterer can be found at https://github.com/coreos/cargo-vendor-filterer
# It is also part of the Rust SDK extension.
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu --platform=aarch64-unknown-linux-gnu "$VENDOR_CRATES" > "$DIST"/.cargo/config.toml

set -- "$VENDOR_CRATES"/gettext-sys/gettext-*.tar.*
TARBALL_PATH=$1
TARBALL_NAME=$(basename "$TARBALL_PATH")
rm -f "$TARBALL_PATH"
# remove the tarball from checksums
cargo_checksum="$VENDOR_CRATES/gettext-sys/.cargo-checksum.json"
tmp_f=$(mktemp --tmpdir="$VENDOR_CRATES/gettext-sys" -t)
jq -c "del(.files[\"$TARBALL_NAME\"])" "$cargo_checksum" > "$tmp_f"
mv -f "$tmp_f" "$cargo_checksum"
# Don't combine the previous and this line with a pipe because we can't catch
# errors with "set -o pipefail"
sed -i "s/^directory = \".*\"/directory = \"$VENDOR_CRATES\"/g" "$DIST/.cargo/config.toml"
# Move the registry crates into dist tarball directory
mv "$VENDOR_CRATES" "$DIST"
