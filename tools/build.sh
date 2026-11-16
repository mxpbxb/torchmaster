#!/usr/bin/env bash
# Builds and checks both parts of Torchmaster for Horizon:
#   target/torchmaster.hmod          content (blocks, items, art, recipes)
#   target/torchmaster_native.hnmod  native code (built into the game by the launcher)
# Runs the native unit tests against the pinned Horizon checkout first.
#   JAR=<unpacked Torchmaster jar> bash tools/build.sh   (JAR regenerates content/ first)
set -euo pipefail
. "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HORIZON="${HORIZON:-$HOME/horizon-dev/wt-census}"          # detached checkout of 1fe4e6fe
TARGET="${CHECK_TARGET:-$HOME/horizon-dev/check-target-pinned}"
TOOL="${HORIZON_MOD_TOOL:-$HOME/horizon-dev/forks/Lunatic/target/tools/debug/cargo-horizon-mod}"
mkdir -p "$ROOT/target"

want=$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' "$ROOT/native/horizon-mixin.toml")
have=$(git -C "$HORIZON" rev-parse HEAD)
[ "$want" = "$have" ] || { echo "Horizon checkout is $have, the mod is pinned to $want" >&2; exit 1; }

if [ -n "${JAR:-}" ]; then
  echo "== content from $JAR"
  (cd /tmp && python3 -I "$ROOT/tools/gen_content.py" "$JAR" "$ROOT/content")
fi

echo "== native unit tests (Horizon $have)"
cd "$ROOT/native/api"
cp "$HORIZON/Cargo.lock" Cargo.lock
cargo --config "patch.\"https://github.com/horizon-rust/horizon\".horizon-client.path=\"$HORIZON/crates/horizon-client\"" \
      --config "build.target-dir=\"$TARGET\"" test --lib -j 2
rm -f Cargo.lock

echo "== content"
cd "$ROOT/content"
"$TOOL" horizon-mod build
"$TOOL" horizon-mod check target/horizon-mod/torchmaster.hmod
cp target/horizon-mod/torchmaster.hmod "$ROOT/target/"

echo "== native bundle"
python3 -I "$ROOT/tools/packnative.py" "$ROOT/native" "$ROOT/target/torchmaster_native.hnmod"
"$TOOL" horizon-mod check "$ROOT/target/torchmaster_native.hnmod"
sha256sum "$ROOT/target/"*.hmod "$ROOT/target/"*.hnmod
