//! Torchmaster's settings, as its NeoForge config has them, in `config/torchmaster-horizon.toml`.

use std::collections::HashSet;

/// What a server writes when it finds no settings file: every setting at Torchmaster's default.
pub const DEFAULT_FILE: &str = r#"# Torchmaster for Horizon. Changes take effect when the game starts again.

# The radius of the mega torch in each direction (cube) with the torch at its center
megaTorchRadius = 64

# The radius of the dread lamp in each direction (cube) with the lamp at its center
dreadLampRadius = 64

# If enabled, the mega torch blocks village sieges from zombies
blockVillageSieges = true

# Overrides for the lists of entities each light blocks.
# The + prefix adds an entity to the list (its spawns are denied),
# the - prefix removes it (its spawns are allowed).
# Block zombies: ["+minecraft:zombie"]   Allow creepers: ["-minecraft:creeper"]
megaTorchEntityBlockListOverrides = []
# Block squid: ["+minecraft:squid"]   Allow pigs: ["-minecraft:pig"]
dreadLampEntityBlockListOverrides = []

# The radius in which the feral flare lantern tries to place lights (1 to 127)
feralFlareRadius = 16

# How often the lantern tries to place a light: 1 is every tick, 10 every 10th tick
feralFlareTickRate = 5

# The light level below which the lantern places a light (0 to 15)
feralFlareMinLightLevel = 10

# The most invisible lights one lantern places. 0 places none.
feralFlareLanternLightCountHardcap = 255
"#;

/// The `+` and `-` entries of a block list override.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    pub add: HashSet<String>,
    pub remove: HashSet<String>,
}

impl Overrides {
    /// `["+minecraft:pig", "-gaia:dryad"]`; entries that aren't `[+-]namespace:path` are skipped,
    /// as Torchmaster skips them.
    pub fn parse(list: &str) -> Self {
        let mut out = Self::default();
        let inner = list.trim().trim_start_matches('[').trim_end_matches(']');
        for raw in inner.split(',') {
            let entry = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if !valid(entry) {
                continue;
            }
            let (sign, id) = entry.split_at(1);
            match sign {
                "+" => {
                    out.remove.remove(id);
                    out.add.insert(id.to_owned());
                }
                _ => {
                    out.add.remove(id);
                    out.remove.insert(id.to_owned());
                }
            }
        }
        out
    }
}

/// Torchmaster's filter pattern, `[+-][a-z0-9_-]+:[a-z0-9_-]+`.
fn valid(entry: &str) -> bool {
    let Some(rest) = entry.strip_prefix(['+', '-']) else { return false };
    let Some((ns, path)) = rest.split_once(':') else { return false };
    let ok = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    ok(ns) && ok(path)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mega_radius: i32,
    pub dread_radius: i32,
    pub block_village_sieges: bool,
    pub mega_overrides: Overrides,
    pub dread_overrides: Overrides,
    pub flare_radius: i32,
    pub flare_tick_rate: u32,
    pub flare_min_light: u8,
    pub flare_hardcap: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mega_radius: 64,
            dread_radius: 64,
            block_village_sieges: true,
            mega_overrides: Overrides::default(),
            dread_overrides: Overrides::default(),
            flare_radius: 16,
            flare_tick_rate: 5,
            flare_min_light: 10,
            flare_hardcap: 255,
        }
    }
}

impl Config {
    /// `key = value` lines; `#` starts a comment. Unknown keys and bad values keep the default,
    /// and numbers are held to Torchmaster's ranges.
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            let int = |lo: i64, hi: i64| value.parse::<i64>().ok().map(|v| v.clamp(lo, hi));
            match key {
                "megaTorchRadius" => c.mega_radius = int(0, i32::MAX as i64).map_or(c.mega_radius, |v| v as i32),
                "dreadLampRadius" => c.dread_radius = int(0, i32::MAX as i64).map_or(c.dread_radius, |v| v as i32),
                "blockVillageSieges" => c.block_village_sieges = value.parse().unwrap_or(c.block_village_sieges),
                "megaTorchEntityBlockListOverrides" => c.mega_overrides = Overrides::parse(value),
                "dreadLampEntityBlockListOverrides" => c.dread_overrides = Overrides::parse(value),
                "feralFlareRadius" => c.flare_radius = int(1, 127).map_or(c.flare_radius, |v| v as i32),
                "feralFlareTickRate" => c.flare_tick_rate = int(1, u32::MAX as i64).map_or(c.flare_tick_rate, |v| v as u32),
                "feralFlareMinLightLevel" => c.flare_min_light = int(0, 15).map_or(c.flare_min_light, |v| v as u8),
                "feralFlareLanternLightCountHardcap" => c.flare_hardcap = int(0, 32767).map_or(c.flare_hardcap, |v| v as usize),
                _ => {}
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_file_reads_as_the_defaults() {
        assert_eq!(Config::parse(DEFAULT_FILE), Config::default());
    }

    #[test]
    fn settings_are_read_and_held_to_their_ranges() {
        let c = Config::parse("megaTorchRadius = 32\nferalFlareRadius = 500 # too far\nferalFlareMinLightLevel = -3\nblockVillageSieges = false\nnonsense = 1");
        assert_eq!(c.mega_radius, 32);
        assert_eq!(c.flare_radius, 127);
        assert_eq!(c.flare_min_light, 0);
        assert!(!c.block_village_sieges);
        assert_eq!(c.dread_radius, 64);
    }

    #[test]
    fn overrides_add_and_remove_and_skip_bad_entries() {
        let o = Overrides::parse(r#"["+minecraft:pig", "-gaia:dryad", "zombie", "+Bad:Name", '-minecraft:creeper']"#);
        assert!(o.add.contains("minecraft:pig"));
        assert!(o.remove.contains("gaia:dryad"));
        assert!(o.remove.contains("minecraft:creeper"));
        assert_eq!(o.add.len() + o.remove.len(), 3);
        // The last word on an entity wins.
        let o = Overrides::parse(r#"["+minecraft:pig", "-minecraft:pig"]"#);
        assert!(o.remove.contains("minecraft:pig") && !o.add.contains("minecraft:pig"));
    }
}
