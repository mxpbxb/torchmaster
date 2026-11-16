"""Writes the content mod `torchmaster` (content/) from Torchmaster's jar, unpacked in <jar dir>.

    python3 -I tools/gen_content.py <unpacked jar dir> <content dir>

The jar is read as data only: textures, models, names and loot are copied; blocks, items, recipes
and tags are written for Horizon 26.3. Torchmaster is MIT licensed (Xalcon).
"""
import json
import shutil
import sys
from pathlib import Path

jar = Path(sys.argv[1])
out = Path(sys.argv[2])
NS = "torchmaster"
A = jar / "assets" / NS

for sub in ("assets", "data"):
    shutil.rmtree(out / sub, ignore_errors=True)


def write(rel, obj):
    p = out / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(obj, (dict, list)):
        p.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")
    else:
        p.write_bytes(obj)


VERSION = "21.1.13-horizon.1"
(out / "horizon-mod.toml").write_text(f'''id = "{NS}"
name = "Torchmaster by Xalcon (assets)"
version = "{VERSION}"
description = "Horizon port of Torchmaster by Xalcon (MIT): the Mega Torch, Dread Lamp, Feral Flare Lantern and Frozen Pearl, with their art and recipes. Needs torchmaster_native for what they do."
authors = ["Xalcon (Torchmaster)", "mxpbxb (Horizon port)"]
api = "0.2"
side = "both"
capabilities = ["registries"]
''', encoding="utf-8")

# ---- Blocks and items ----
H = f"data/{NS}/horizon"
write(f"{H}/block/megatorch.json", {
    # Glass: no tool needed, lets skylight through and occludes nothing, as Torchmaster's block.
    "template": "minecraft:glass",
    "properties": {"hardness": 1.0, "explosion_resistance": 1.0, "sounds": "minecraft:oak_planks", "light": 15},
    "shapes": {"default": [[0.375, 0, 0.375, 0.625, 1, 0.625]]},
    "item": True,
})
write(f"{H}/block/dreadlamp.json", {
    "template": "minecraft:glass",
    "properties": {"hardness": 1.0, "explosion_resistance": 1.0, "sounds": "minecraft:oak_planks", "light": 15},
    "shapes": {"default": [[0.0625, 0, 0.0625, 0.9375, 0.9375, 0.9375]]},
    "item": True,
})
write(f"{H}/block/feral_flare_lantern.json", {
    # The end rod: six facings, set from the face clicked, and no support needed.
    "template": "minecraft:end_rod",
    "properties": {"hardness": 1.0, "explosion_resistance": 1.0, "sounds": "minecraft:lantern", "light": 15},
    "shapes": {"default": [[0.25, 0, 0.25, 0.75, 1, 0.75]]},
    "item": True,
})
write(f"{H}/block/invisible_light.json", {
    # Vanilla's light block: invisible, no collision, replaceable, lit by its level (15).
    "template": "minecraft:light",
    "properties": {"light": 15},
    "shapes": {"default": []},
    "collision_shapes": {"default": []},
    "item": False,
})
write(f"{H}/item/frozen_pearl.json", {"template": "minecraft:paper"})
write(f"{H}/creative_tab/{NS}.json", {
    "title": "Torchmaster",
    "icon": f"{NS}:megatorch",
    "items": [f"{NS}:megatorch", f"{NS}:dreadlamp", f"{NS}:feral_flare_lantern", f"{NS}:frozen_pearl"],
})

# ---- Recipes ----
# Each needs four of Arcadian Dream's Max Point Items (in the corners) and diamonds, around the
# heart of Torchmaster's own recipe. Without the arcadiandream mod they can't be made.
MAX_POINT = "arcadiandream:max_point_item"


def shaped(pattern, key, result):
    key = {"M": MAX_POINT, "D": "minecraft:diamond", **key}
    return {"type": "minecraft:crafting_shaped", "category": "misc", "pattern": pattern, "key": key, "result": {"id": result}}


write(f"data/{NS}/recipe/megatorch.json", shaped(["MTM", "DLD", "MLM"], {
    "T": "minecraft:torch", "L": "#minecraft:logs"}, f"{NS}:megatorch"))
write(f"data/{NS}/recipe/dreadlamp.json", shaped(["MOM", "DGD", "MBM"], {
    "O": "minecraft:obsidian", "G": "minecraft:glowstone", "B": "minecraft:black_dye"}, f"{NS}:dreadlamp"))
write(f"data/{NS}/recipe/feral_flare_lantern.json", shaped(["MIM", "DgD", "MIM"], {
    "I": "minecraft:gold_ingot", "g": "minecraft:glowstone_dust"}, f"{NS}:feral_flare_lantern"))
write(f"data/{NS}/recipe/frozen_pearl.json", shaped(["MIM", "IEI", "MDM"], {
    "E": "minecraft:ender_pearl", "I": ["minecraft:ice", "minecraft:packed_ice"]}, f"{NS}:frozen_pearl"))

# ---- Loot and tags ----
for p in sorted((jar / "data" / NS / "loot_table" / "blocks").glob("*.json")):
    write(f"data/{NS}/loot_table/blocks/{p.name}", json.loads(p.read_text()))
write("data/minecraft/tags/block/mineable/axe.json", {"values": [f"{NS}:megatorch"]})
write("data/minecraft/tags/block/mineable/pickaxe.json", {"values": [f"{NS}:feral_flare_lantern", f"{NS}:dreadlamp"]})

# ---- Assets ----
for p in sorted((A / "models").rglob("*.json")):
    write(f"assets/{NS}/models/{p.relative_to(A / 'models').as_posix()}", json.loads(p.read_text()))
for p in sorted((A / "textures").rglob("*.png")):
    write(f"assets/{NS}/textures/{p.relative_to(A / 'textures').as_posix()}", p.read_bytes())
for p in sorted((A / "lang").glob("*.json")):
    write(f"assets/{NS}/lang/{p.name}", json.loads(p.read_text(encoding="utf-8")))
for name in ("megatorch", "dreadlamp", "invisible_light"):
    write(f"assets/{NS}/blockstates/{name}.json", json.loads((A / "blockstates" / f"{name}.json").read_text()))

# Torchmaster's lantern faces where its stand points (the face clicked, turned round); the end rod
# it copies faces the face clicked. Its stand parts are keyed by the end rod's facing.
OPPOSITE = {"north": "south", "south": "north", "east": "west", "west": "east", "up": "down", "down": "up"}
lantern = json.loads((A / "blockstates" / "feral_flare_lantern.json").read_text())
for part in lantern["multipart"]:
    if "when" in part:
        part["when"] = {"facing": OPPOSITE[part["when"]["facing"]]}
write(f"assets/{NS}/blockstates/feral_flare_lantern.json", lantern)

for name, model in (("megatorch", "block/megatorch"), ("dreadlamp", "block/dreadlamp"),
                    ("feral_flare_lantern", "block/feral_flare_lantern"), ("frozen_pearl", "item/frozen_pearl")):
    write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:{model}"}})

print("content written:", sum(1 for _ in out.rglob("*") if _.is_file()), "files")
