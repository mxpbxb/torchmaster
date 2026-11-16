"""Writes <id>.hnmod for the fork: horizon-native.json plus mixins/{mod_id}/ (every helper file)."""
import json
import re
import sys
import zipfile
from pathlib import Path

native = Path(sys.argv[1])
out = Path(sys.argv[2])
toml = (native / "horizon-mixin.toml").read_text()
rev = re.search(r'horizon_revision = "([0-9a-f]{40})"', toml).group(1)
mod_id = re.search(r'^id = "([^"]+)"', toml, re.M).group(1)
version = re.search(r'^version = "([^"]+)"', toml, re.M).group(1)
manifest = {"schema": 1, "id": mod_id, "version": version, "game": "26.3", "horizon": rev,
            "mixins": "e8cd0aa38cffef49cb2e088ef716baa107148c02"}
helpers = sorted(p.name for p in native.glob("*.rs"))
# Every helper and module the manifest names must be packed.
for named in re.findall(r'^(?:helper|source) = "([^"]+)"', toml, re.M):
    assert named in helpers, named
files = ["horizon-mixin.toml", *helpers, "api/Cargo.toml", *sorted(str(p.relative_to(native)) for p in (native / "api" / "src").glob("*.rs"))]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    def add(name, data):
        entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
        entry.compress_type = zipfile.ZIP_DEFLATED
        entry.external_attr = 0o100644 << 16
        archive.writestr(entry, data)
    add("horizon-native.json", json.dumps(manifest, indent=2).encode())
    for f in files:
        add(f"mixins/{mod_id}/{f}", (native / f).read_bytes())
print("wrote", out, out.stat().st_size, "version", version, "files", len(files))
