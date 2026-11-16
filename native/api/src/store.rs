//! The world's mega torches and dread lamps, kept in `<world>/torchmaster/lights.tsv` as
//! Torchmaster keeps them in the level's saved data, so a light blocks spawns in chunks around it
//! whether or not its own chunk is loaded.

use crate::config::{Config, Overrides};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MegaTorch,
    DreadLamp,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::MegaTorch => "megatorch",
            Kind::DreadLamp => "dreadlamp",
        }
    }

    pub fn block(self) -> &'static str {
        match self {
            Kind::MegaTorch => crate::MEGATORCH,
            Kind::DreadLamp => crate::DREADLAMP,
        }
    }

    pub fn of_block(block: &str) -> Option<Self> {
        match block {
            crate::MEGATORCH => Some(Kind::MegaTorch),
            crate::DREADLAMP => Some(Kind::DreadLamp),
            _ => None,
        }
    }

    fn of_name(name: &str) -> Option<Self> {
        [Kind::MegaTorch, Kind::DreadLamp].into_iter().find(|k| k.name() == name)
    }

    pub fn radius(self, config: &Config) -> i32 {
        match self {
            Kind::MegaTorch => config.mega_radius,
            Kind::DreadLamp => config.dread_radius,
        }
    }

    /// Whether this light keeps `entity`, of spawn category `category`, from spawning. The mega
    /// torch's list is every entity that isn't friendly (the monsters); the dread lamp's every
    /// friendly one but the `misc` category (animals, bats, squid, fish, axolotls). The overrides
    /// add to and take from the lists.
    /// The dread lamp is turned off in this port: it blocks nothing, placed ones included.
    pub fn blocks(self, category: &str, entity: &str, config: &Config) -> bool {
        let (overrides, listed): (&Overrides, bool) = match self {
            Kind::MegaTorch => (&config.mega_overrides, category == "monster"),
            Kind::DreadLamp => return false,
        };
        if overrides.remove.contains(entity) {
            return false;
        }
        listed || overrides.add.contains(entity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub kind: Kind,
    pub dimension: String,
    pub pos: [i32; 3],
}

impl Light {
    /// Torchmaster's cubic range: `pos` is within `radius` of the block's middle on every axis.
    pub fn reaches(&self, pos: [f64; 3], radius: i32) -> bool {
        (0..3).all(|i| (pos[i] - (f64::from(self.pos[i]) + 0.5)).abs() <= f64::from(radius))
    }
}

#[derive(Debug, Default)]
pub struct Store {
    pub lights: Vec<Light>,
    /// The next light the clean-up looks at.
    pub cursor: usize,
}

impl Store {
    pub fn decode(text: &str) -> Self {
        let lights = text
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\t').collect();
                let [kind, dim, x, y, z] = f.as_slice() else { return None };
                Some(Light {
                    kind: Kind::of_name(kind)?,
                    dimension: (*dim).to_owned(),
                    pos: [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
                })
            })
            .collect();
        Self { lights, cursor: 0 }
    }

    pub fn encode(&self) -> String {
        self.lights
            .iter()
            .map(|l| format!("{}\t{}\t{}\t{}\t{}\n", l.kind.name(), l.dimension, l.pos[0], l.pos[1], l.pos[2]))
            .collect()
    }

    /// Adds a light; false when it was already known.
    pub fn add(&mut self, kind: Kind, dimension: &str, pos: [i32; 3]) -> bool {
        if self.lights.iter().any(|l| l.kind == kind && l.dimension == dimension && l.pos == pos) {
            return false;
        }
        self.remove(dimension, pos);
        self.lights.push(Light { kind, dimension: dimension.to_owned(), pos });
        true
    }

    /// Forgets whatever light was at `pos`; false when none was.
    pub fn remove(&mut self, dimension: &str, pos: [i32; 3]) -> bool {
        let before = self.lights.len();
        self.lights.retain(|l| !(l.dimension == dimension && l.pos == pos));
        before != self.lights.len()
    }

    /// Whether a light of `kind` in `dimension` reaches `pos`.
    pub fn reaches(&self, kind: Kind, dimension: &str, pos: [f64; 3], config: &Config) -> bool {
        let radius = kind.radius(config);
        self.lights.iter().any(|l| l.kind == kind && l.dimension == dimension && l.reaches(pos, radius))
    }

    /// Whether a light near `pos` keeps `entity` (of `category`) from spawning there.
    pub fn blocks_spawn(&self, dimension: &str, category: &str, entity: &str, pos: [f64; 3], config: &Config) -> bool {
        [Kind::MegaTorch, Kind::DreadLamp]
            .into_iter()
            .any(|k| k.blocks(category, entity, config) && self.reaches(k, dimension, pos, config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OW: &str = "minecraft:overworld";

    #[test]
    fn the_store_survives_a_round_trip() {
        let mut s = Store::default();
        assert!(s.add(Kind::MegaTorch, OW, [1, 64, -3]));
        assert!(s.add(Kind::DreadLamp, "minecraft:the_nether", [-100, 30, 7]));
        assert!(!s.add(Kind::MegaTorch, OW, [1, 64, -3]));
        let back = Store::decode(&s.encode());
        assert_eq!(back.lights, s.lights);
        assert!(back.lights.len() == 2);
        assert_eq!(Store::decode("junk\nmegatorch\tminecraft:overworld\t1\t2\n").lights, []);
    }

    #[test]
    fn a_light_placed_over_another_replaces_it() {
        let mut s = Store::default();
        s.add(Kind::MegaTorch, OW, [0, 0, 0]);
        s.add(Kind::DreadLamp, OW, [0, 0, 0]);
        assert_eq!(s.lights.len(), 1);
        assert_eq!(s.lights[0].kind, Kind::DreadLamp);
        assert!(s.remove(OW, [0, 0, 0]));
        assert!(!s.remove(OW, [0, 0, 0]));
    }

    #[test]
    fn the_range_is_a_cube_about_the_block_middle() {
        let l = Light { kind: Kind::MegaTorch, dimension: OW.into(), pos: [0, 64, 0] };
        assert!(l.reaches([64.5, 64.5, -63.5], 64));
        assert!(l.reaches([64.4, 0.6, 0.0], 64));
        assert!(!l.reaches([64.6, 64.5, 0.5], 64));
        assert!(!l.reaches([0.5, 128.6, 0.5], 64));
    }

    #[test]
    fn each_light_blocks_its_own_list() {
        let c = Config::default();
        assert!(Kind::MegaTorch.blocks("monster", "minecraft:zombie", &c));
        assert!(!Kind::MegaTorch.blocks("creature", "minecraft:pig", &c));
        assert!(!Kind::DreadLamp.blocks("creature", "minecraft:pig", &c));
        // The dread lamp is turned off: it blocks nothing.
        assert!(!Kind::DreadLamp.blocks("water_ambient", "minecraft:cod", &c));
        assert!(!Kind::DreadLamp.blocks("ambient", "minecraft:bat", &c));
        assert!(!Kind::DreadLamp.blocks("monster", "minecraft:zombie", &c));
        assert!(!Kind::DreadLamp.blocks("misc", "minecraft:iron_golem", &c));
    }

    #[test]
    fn overrides_change_the_lists() {
        let c = Config::parse("megaTorchEntityBlockListOverrides = [\"-minecraft:creeper\", \"+minecraft:pig\"]");
        assert!(!Kind::MegaTorch.blocks("monster", "minecraft:creeper", &c));
        assert!(Kind::MegaTorch.blocks("creature", "minecraft:pig", &c));
        assert!(Kind::MegaTorch.blocks("monster", "minecraft:zombie", &c));
    }

    #[test]
    fn spawns_are_blocked_only_near_the_right_light_in_its_dimension() {
        let c = Config::default();
        let mut s = Store::default();
        s.add(Kind::MegaTorch, OW, [0, 64, 0]);
        assert!(s.blocks_spawn(OW, "monster", "minecraft:zombie", [10.5, 60.0, 10.5], &c));
        assert!(!s.blocks_spawn(OW, "monster", "minecraft:zombie", [100.5, 60.0, 10.5], &c));
        assert!(!s.blocks_spawn("minecraft:the_nether", "monster", "minecraft:zombie", [0.5, 60.0, 0.5], &c));
        assert!(!s.blocks_spawn(OW, "creature", "minecraft:cow", [0.5, 60.0, 0.5], &c));
        // A dread lamp, turned off, still lets cows spawn.
        s.add(Kind::DreadLamp, OW, [0, 64, 0]);
        assert!(!s.blocks_spawn(OW, "creature", "minecraft:cow", [0.5, 60.0, 0.5], &c));
    }
}
