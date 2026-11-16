# Torchmaster for Horizon

A port of [Torchmaster](https://github.com/Xalcon/TorchMaster) by **Xalcon** (MIT) to Horizon (Minecraft 26.3).

- `content/` — the content mod `torchmaster` (`.hmod`): Mega Torch, Dread Lamp, Feral Flare Lantern,
  the invisible light, the Frozen Pearl, with Torchmaster's art, names, loot and recipes.
- `native/` — the native mod `torchmaster_native` (`.hnmod`), pinned to Horizon `1fe4e6fe`.

## What works

- **Mega Torch**: no natural spawns of hostile mobs within 64 blocks in each direction (a cube),
  no phantoms there, no zombie village sieges.
- **Dread Lamp**: the same for passive mobs (animals, bats, squid, fish, axolotls).
- **Feral Flare Lantern**: every 5 ticks tries a random spot within 16 blocks; if it is air and its
  block light is under 10, places an invisible light there (up to 255). Breaking it removes them.
- **Frozen Pearl**: use it to clear every invisible light within 15 blocks.
- Right-click a torch or lamp with an empty hand to see its range. Item tooltips as in Torchmaster.
- Settings: `config/torchmaster-horizon.toml`, Torchmaster's names and defaults, including the
  `+`/`-` entity block list overrides.

## Differences from Torchmaster

- Lights are tracked in `<world>/torchmaster/lights.tsv`.
- No range display screen, no lantern settings screen (line of sight is always off, Torchmaster's
  default), no `/torchmaster` command, no flame particles or waterlogging on the torch and lamp.
- Every recipe needs four of Arcadian Dream's Max Point Items (`arcadiandream:max_point_item`, the
  corners) and diamonds, around the heart of Torchmaster's recipe:
  Mega Torch `MTM/DLD/MLM` (torch, logs), Dread Lamp `MOM/DGD/MBM` (obsidian, glowstone, black dye),
  Feral Flare Lantern `MIM/DgD/MIM` (gold ingots, glowstone dust), Frozen Pearl `MIM/IEI/MDM`
  (ice or packed ice, ender pearl). Without the `arcadiandream` mod they can't be crafted.
- Phantoms are kept out by refusing a new phantom near a torch, which also stops spawn eggs there.
- A lantern removed by anything but a player (explosion, piston) leaves its lights; the pearl clears them.

## Build

```bash
JAR=<unpacked torchmaster jar> bash tools/build.sh
```
