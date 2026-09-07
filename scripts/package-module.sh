#!/usr/bin/env bash
# Build a module to wasm, place it at its versioned artifact path, and pin its
# SHA-256 into the manifest - the three steps slim's registry layout needs.
#
# Usage: scripts/package-module.sh <module-dir>
#   e.g. scripts/package-module.sh modules/_template
#
# Reads `id` and `version` from the module's manifest.json, builds the release
# wasm, copies it to modules/<id>/<version>/module.wasm, and rewrites the
# manifest's artifact.path + artifact.sha256 to match. Then add/update the
# module's row in index.json and commit.
set -euo pipefail

dir="${1:?usage: package-module.sh <module-dir>}"
manifest="$dir/manifest.json"
[[ -f "$manifest" ]] || { echo "no manifest at $manifest" >&2; exit 1; }

id=$(python3 -c "import json,sys;print(json.load(open('$manifest'))['id'])")
version=$(python3 -c "import json,sys;print(json.load(open('$manifest'))['version'])")
echo "packaging $id v$version"

# Build. --target-dir keeps artifacts out of the module dir (it is gitignored).
( cd "$dir" && cargo build --release --target wasm32-unknown-unknown )
wasm=$(find "$dir/target/wasm32-unknown-unknown/release" -maxdepth 1 -name '*.wasm' | head -1)
[[ -f "$wasm" ]] || { echo "no .wasm built under $dir/target" >&2; exit 1; }

# Place it at the versioned artifact path the manifest will point at.
artifact_path="modules/$id/$version/module.wasm"
mkdir -p "$(dirname "$artifact_path")"
cp "$wasm" "$artifact_path"
sha=$(sha256sum "$artifact_path" | cut -d' ' -f1)
echo "sha256: $sha"

# Pin path + sha into the manifest.
python3 - "$manifest" "$artifact_path" "$sha" <<'PY'
import json, sys
manifest, path, sha = sys.argv[1], sys.argv[2], sys.argv[3]
m = json.load(open(manifest))
m["artifact"]["path"] = path
m["artifact"]["sha256"] = sha
json.dump(m, open(manifest, "w"), indent=2)
open(manifest, "a").write("\n")
PY

echo "done. Now add/update $id in index.json and commit:"
echo "  $artifact_path"
echo "  $manifest"
