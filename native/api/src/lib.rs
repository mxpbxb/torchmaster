//! Torchmaster for Horizon: a port of Torchmaster by Xalcon (MIT).
//!
//! - The **Mega Torch** keeps hostile monsters from spawning naturally within 64 blocks of it in
//!   each direction (a cube), stops phantoms there and village sieges by zombies.
//! - The **Dread Lamp** does the same for passive mobs: animals, bats, squid, fish, axolotls.
//! - The **Feral Flare Lantern** places invisible lights in dark places within 16 blocks of it,
//!   up to 255, and takes them away when it is broken.
//! - The **Frozen Pearl** clears every invisible light within 15 blocks of the player using it.
//!
//! The blocks, items, art and recipes are the content mod `torchmaster`. Settings are in
//! `config/torchmaster-horizon.toml`, with Torchmaster's names and defaults.

mod config;
mod lantern;
mod store;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use horizon_client::mod_api::{
    BlockEntityEvent, BlockEntityTickEvent, BlockEntityTicksEvent, Compound, DestroyBlockEvent,
    EntityJoinLevelEvent, Flow, Hooks, Host, NativeHeightmap, PlaceBlockEvent, PotentialSpawnsEvent,
    RightClickBlockEvent, ServerTickEvent, ServerTickPhase, Text, TooltipEvent, UseItemEvent,
    VillageSiegeEvent,
};

use config::Config;
use lantern::Rng;
use store::{Kind, Store};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Mod)
}

struct Mod;

pub const MEGATORCH: &str = "torchmaster:megatorch";
pub const DREADLAMP: &str = "torchmaster:dreadlamp";
pub const LANTERN: &str = "torchmaster:feral_flare_lantern";
pub const LIGHT: &str = "torchmaster:invisible_light";
pub const PEARL: &str = "torchmaster:frozen_pearl";
const PHANTOM: &str = "minecraft:phantom";
/// How far the frozen pearl reaches, in each direction.
const PEARL_REACH: i32 = 15;
/// `SoundSource.BLOCKS`.
const BLOCKS: usize = 4;

// ---- Settings ----

static CONFIG: OnceLock<Arc<Config>> = OnceLock::new();

/// The settings, read once from `config/torchmaster-horizon.toml` (written with the defaults when
/// missing).
fn config(host: &(impl Host + ?Sized)) -> Arc<Config> {
    CONFIG
        .get_or_init(|| {
            let Ok(dir) = host.config_dir() else { return Arc::new(Config::default()) };
            let path = dir.join("torchmaster-horizon.toml");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                let _ = std::fs::write(&path, config::DEFAULT_FILE);
                config::DEFAULT_FILE.to_owned()
            });
            Arc::new(Config::parse(&text))
        })
        .clone()
}

// ---- The world's lights ----

static STORE: LazyLock<Mutex<Option<(PathBuf, Store)>>> = LazyLock::new(Default::default);

fn store_path(host: &(impl Host + ?Sized)) -> Option<PathBuf> {
    Some(host.world_dir()?.join("torchmaster").join("lights.tsv"))
}

/// Runs `f` on the open world's lights, loading them first if another world's are loaded; saves
/// them when `f` says it changed them.
fn with_store<R>(host: &(impl Host + ?Sized), f: impl FnOnce(&mut Store) -> (R, bool)) -> Option<R> {
    let path = store_path(host)?;
    let mut guard = STORE.lock().ok()?;
    if guard.as_ref().is_none_or(|(p, _)| *p != path) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        *guard = Some((path.clone(), Store::decode(&text)));
    }
    let (_, store) = guard.as_mut()?;
    let (result, changed) = f(store);
    if changed {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, store.encode());
    }
    Some(result)
}

fn register(host: &(impl Host + ?Sized), kind: Kind, dimension: &str, pos: [i32; 3]) {
    with_store(host, |s| ((), s.add(kind, dimension, pos)));
}

fn unregister(host: &(impl Host + ?Sized), dimension: &str, pos: [i32; 3]) {
    with_store(host, |s| ((), s.remove(dimension, pos)));
}

/// Server ticks counted since the game started.
static TICKS: AtomicU64 = AtomicU64::new(0);

fn middle(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

fn dimension_of(host: &dyn Host) -> String {
    host.acting_player().map_or_else(|| host.dimension().to_owned(), |p| p.dimension)
}

// ---- The lantern ----

static RNG: LazyLock<Mutex<Rng>> = LazyLock::new(|| {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    Mutex::new(Rng::new(seed))
});

/// What a lantern keeps between ticks without saving it: ticks since it last tried a light, and
/// which of its lights it checks next (Torchmaster's `ticks` and `childLightCheckIndex`).
static LANTERNS: LazyLock<Mutex<HashMap<(String, [i32; 3]), (u32, usize)>>> = LazyLock::new(Default::default);

fn is_air(block: &str) -> bool {
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// Takes away the lights a lantern placed that are still there.
fn remove_lights(host: &mut dyn Host, dimension: &str, lights: &[[i32; 3]]) {
    let Some(air) = host.default_state("minecraft:air") else { return };
    for &p in lights {
        if host.block(p) == LIGHT {
            host.set_block(dimension, p, air, 3);
        }
    }
}

fn lantern_tick(host: &mut dyn Host, pos: [i32; 3], nbt: &Compound) -> Option<Vec<[i32; 3]>> {
    let config = config(host);
    let dimension = host.dimension().to_owned();
    let level = host.level(&dimension)?;
    let mut lights = lantern::decode(nbt);
    let mut changed = false;
    let key = (dimension.clone(), pos);
    let (ticks, check) = {
        let mut lanterns = LANTERNS.lock().ok()?;
        let state = lanterns.entry(key.clone()).or_insert((0, 0));
        state.0 += 1;
        *state
    };
    // One of its lights a tick: one that is gone (a block put there, a pearl) is forgotten.
    let mut next_check = check;
    if !lights.is_empty() {
        let i = check % lights.len();
        let p = lights[i];
        if host.level_is_loaded(level, p) && host.block(p) != LIGHT {
            lights.remove(i);
            changed = true;
            next_check = i;
        } else {
            next_check = i + 1;
        }
    }
    let mut next_ticks = ticks;
    if ticks >= config.flare_tick_rate && lights.len() < config.flare_hardcap {
        next_ticks = 0;
        let (min_y, height) = host.world_height();
        let target = {
            let mut rng = RNG.lock().ok()?;
            let surface = |x: i32, z: i32| {
                host.surface_heights(&dimension, &[(x, z)], NativeHeightmap::WorldSurface).ok().and_then(|v| v.first().copied().flatten())
            };
            lantern::target(pos, config.flare_radius, &mut |n| rng.below(n), surface, min_y, min_y + height)
        };
        if let Some(t) = target
            && host.level_is_loaded(level, t)
            && is_air(&host.block(t))
            && host.light(t).0 < config.flare_min_light
            && let Some(state) = host.default_state(LIGHT)
            && host.set_block(&dimension, t, state, 3)
        {
            lights.push(t);
            changed = true;
        }
    }
    if let Ok(mut lanterns) = LANTERNS.lock() {
        lanterns.insert(key, (next_ticks, next_check));
    }
    changed.then_some(lights)
}

// ---- Tooltips ----

fn tooltip_for(item: &str) -> Option<&'static str> {
    Some(match item {
        MEGATORCH => "Prevents natural spawning of hostile monsters in a big radius around the torch",
        DREADLAMP => "Prevents natural spawning of passive animals like squids, bats or ocelots in a big radius around the lamp",
        LANTERN => "Places invisible lights",
        PEARL => "Clears residual lights from a Feral Flare Lantern",
        _ => return None,
    })
}

impl Hooks for Mod {
    /// A mega torch or dread lamp placed starts blocking spawns at once.
    fn place_block(&self, host: &mut dyn Host, event: &mut PlaceBlockEvent<'_>) {
        let Some(kind) = Kind::of_block(&event.stack.item) else { return };
        let dimension = dimension_of(host);
        register(host, kind, &dimension, event.pos);
    }

    /// A light broken stops blocking; a lantern broken takes its lights away with it.
    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        let block = host.block(event.pos);
        let dimension = dimension_of(host);
        if Kind::of_block(&block).is_some() {
            unregister(host, &dimension, event.pos);
        } else if block == LANTERN {
            if let Some(entity) = host.mod_block_entity(event.pos) {
                let lights = lantern::decode(&entity.nbt);
                remove_lights(host, &dimension, &lights);
            }
            if let Ok(mut lanterns) = LANTERNS.lock() {
                lanterns.remove(&(dimension, event.pos));
            }
        }
        Flow::Continue
    }

    /// The lantern has a block entity for its lights; the mega torch and dread lamp have one too,
    /// so one set with a command or found in a world from before is taken in when it ticks.
    fn block_entity(&self, _: &mut dyn Host, event: &mut BlockEntityEvent<'_>) -> Option<(String, Compound)> {
        matches!(event.block, LANTERN | MEGATORCH | DREADLAMP).then(|| (event.block.to_owned(), Compound::new()))
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if matches!(event.block, LANTERN | MEGATORCH | DREADLAMP) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn Host, event: &mut BlockEntityTickEvent<'_>) {
        let id = event.entity.id.clone();
        if let Some(kind) = Kind::of_block(&id) {
            // Every 5 seconds, at its own moment.
            let spread = (event.pos[0] ^ event.pos[2]).unsigned_abs() as u64;
            if (TICKS.load(Ordering::Relaxed) + spread) % 100 == 0 {
                let dimension = host.dimension().to_owned();
                register(host, kind, &dimension, event.pos);
            }
            return;
        }
        if id != LANTERN {
            return;
        }
        if let Some(lights) = lantern_tick(host, event.pos, &event.entity.nbt) {
            event.entity.nbt.insert(lantern::LIGHTS.into(), lantern::encode(&lights));
        }
    }

    /// Once a second, one known light is looked at: one whose block is gone is forgotten
    /// (Torchmaster's `cleanupCheck`).
    fn server_tick(&self, host: &mut dyn Host, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { world_running: true }) {
            return;
        }
        let tick = TICKS.fetch_add(1, Ordering::Relaxed);
        if tick % 20 != 0 {
            return;
        }
        let Some(light) = with_store(host, |s| {
            if s.lights.is_empty() {
                return (None, false);
            }
            s.cursor = (s.cursor + 1) % s.lights.len();
            (Some(s.lights[s.cursor].clone()), false)
        })
        .flatten() else {
            return;
        };
        let Some(level) = host.level(&light.dimension) else { return };
        if host.level_is_loaded(level, light.pos) && host.level_block(level, light.pos).as_deref() != Some(light.kind.block()) {
            unregister(host, &light.dimension, light.pos);
        }
    }

    /// Natural spawning near a light leaves out what the light blocks.
    fn potential_spawns(&self, host: &dyn Host, event: &mut PotentialSpawnsEvent<'_>) -> Flow {
        let config = config(host);
        let at = middle(event.pos);
        let (dimension, category) = (event.dimension, event.category);
        let entries = &mut *event.entries;
        with_store(host, |s| {
            if !s.lights.is_empty() {
                entries.retain(|e| !s.blocks_spawn(dimension, category, &e.entity, at, &config));
            }
            ((), false)
        });
        Flow::Continue
    }

    /// No phantoms near a mega torch (Torchmaster's `PlayerSpawnPhantomsEvent`).
    fn entity_join_level(&self, host: &mut dyn Host, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
        if event.kind != PHANTOM || event.loaded_from_disk {
            return Flow::Continue;
        }
        let config = config(host);
        let blocked = with_store(&*host, |s| (s.blocks_spawn(event.dimension, "monster", PHANTOM, event.pos, &config), false));
        if blocked == Some(true) { Flow::Cancel } else { Flow::Continue }
    }

    /// No village siege near a mega torch.
    fn village_siege(&self, host: &mut dyn Host, event: &mut VillageSiegeEvent<'_>) -> Flow {
        let config = config(host);
        if !config.block_village_sieges {
            return Flow::Continue;
        }
        let near = with_store(&*host, |s| (s.reaches(Kind::MegaTorch, event.dimension, event.attempted_pos, &config), false));
        if near == Some(true) { Flow::Cancel } else { Flow::Continue }
    }

    /// The frozen pearl clears the invisible lights around the player.
    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        if *event.stack.item != *PEARL {
            return Flow::Continue;
        }
        let Some(player) = host.acting_player() else { return Flow::Continue };
        let Some(air) = host.default_state("minecraft:air") else { return Flow::Cancel };
        let at = player.position;
        host.play_sound("minecraft:block.lava.extinguish", at, BLOCKS, 0.7, 0.6);
        let c = at.map(|v| v.floor() as i32);
        let mut cleared = 0;
        for x in c[0] - PEARL_REACH..=c[0] + PEARL_REACH {
            for y in c[1] - PEARL_REACH..=c[1] + PEARL_REACH {
                for z in c[2] - PEARL_REACH..=c[2] + PEARL_REACH {
                    if host.block([x, y, z]) == LIGHT && host.set_block(&player.dimension, [x, y, z], air, 3) {
                        cleared += 1;
                    }
                }
            }
        }
        let line = if cleared == 1 { "Cleared 1 light".to_owned() } else { format!("Cleared {cleared} lights") };
        host.action_bar(player.uuid, Text::literal(line));
        Flow::Cancel
    }

    /// Right-click a mega torch or dread lamp with an empty hand to see how far it reaches.
    fn right_click_block(&self, host: &mut dyn Host, event: &mut RightClickBlockEvent) -> Flow {
        let Some(kind) = Kind::of_block(&host.block(event.pos)) else { return Flow::Continue };
        let Some(player) = host.acting_player() else { return Flow::Continue };
        if player.sneaking || host.held_stack(player.uuid).is_some() {
            return Flow::Continue;
        }
        let config = config(host);
        let name = match kind {
            Kind::MegaTorch => "Mega Torch: hostile mobs",
            Kind::DreadLamp => "Dread Lamp: passive mobs",
        };
        let line = format!("{name} don't spawn within {} blocks in each direction", kind.radius(&config));
        host.action_bar(player.uuid, Text::literal(line));
        Flow::Continue
    }

    fn tooltip(&self, _: &dyn Host, event: &mut TooltipEvent<'_>) {
        if let Some(line) = tooltip_for(&event.stack.item) {
            event.lines.push((Text::legacy(&format!("§7{line}")), u32::MAX));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_has_its_tooltip() {
        for item in [MEGATORCH, DREADLAMP, LANTERN, PEARL] {
            assert!(tooltip_for(item).is_some(), "{item}");
        }
        assert!(tooltip_for(LIGHT).is_none());
    }

    #[test]
    fn the_light_kinds_name_their_blocks() {
        assert_eq!(Kind::of_block(MEGATORCH), Some(Kind::MegaTorch));
        assert_eq!(Kind::of_block(DREADLAMP), Some(Kind::DreadLamp));
        assert_eq!(Kind::of_block(LANTERN), None);
        assert_eq!(Kind::MegaTorch.block(), MEGATORCH);
    }
}
