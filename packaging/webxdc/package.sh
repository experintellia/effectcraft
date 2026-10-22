#!/usr/bin/env bash
# Build EffectCraft as a webxdc app (Delta Chat and other messengers):  $DIST/effectcraft-<version>.xdc
#
# Usage: packaging/webxdc/package.sh [--skip-build]
#
# Needs what packaging/web/package.sh needs. The .xdc is the web build with its own page
# (apps/effectcraft-web/webxdc.html): no service worker or app manifest (a messenger serves the
# app from the .xdc, offline, with no COOP/COEP headers), and the wasm stored gzipped.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"

if [ "${1:-}" != "--skip-build" ]; then
  (cd "$ROOT" && cargo xtask web --small)
fi
WEB="$CARGO_TARGET_DIR/web/dist"
[ -f "$WEB/effectcraft_web_bg.wasm" ] || { echo "error: $WEB/effectcraft_web_bg.wasm missing; run without --skip-build" >&2; exit 1; }

SITE="$CARGO_TARGET_DIR/webxdc-site"
rm -rf "$SITE"
mkdir -p "$SITE"
cp -R "$WEB/." "$SITE/"
# The web build's offline install and page; the messenger provides webxdc.js.
rm -f "$SITE/sw.js" "$SITE/manifest.webmanifest" "$SITE/favicon.svg" "$SITE"/icon-*.png "$SITE/index.html"
cp "$ROOT/assets/app-icon/effectcraft-macos-512.png" "$SITE/icon.png"
# Stored gzipped (zip -n .gz below), gunzipped by the page: Delta Chat inflates zip entries slowly.
gzip -9 -n -f "$SITE/effectcraft_web_bg.wasm"
# The page's loading bar needs the download's size: a messenger may send no Content-Length.
GZ_BYTES=$(( $(wc -c < "$SITE/effectcraft_web_bg.wasm.gz") ))
sed "s/@WASM_GZ_BYTES@/$GZ_BYTES/" "$ROOT/apps/effectcraft-web/webxdc.html" > "$SITE/index.html"
cat > "$SITE/manifest.toml" <<TOML
name = "EffectCraft"
source_code_url = "https://github.com/experintellia/effectcraft"
TOML
copy_docs "$SITE"
# The wasm embeds the OFL fonts (Inter, JetBrains Mono, Noto Serif): their licences, NOTICE and
# the asset attributions go with every copy.
cp "$ROOT/NOTICE" "$ROOT/ATTRIBUTION.md" "$ROOT"/assets/fonts/OFL-*.txt "$SITE/"
OUT="$DIST/effectcraft-$VERSION.xdc"
rm -f "$OUT"
# No -9: in zip 3.0 it overrides -n, which would deflate the .gz again.
(cd "$SITE" && zip -qr -n .gz "$OUT" .)
echo "wrote $OUT"
ls -lh "$OUT"
