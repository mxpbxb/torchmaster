//! The feral flare lantern's sums: where it tries a light, and how it keeps the lights it placed.

use horizon_client::mod_api::{Compound, Tag};

/// The block entity's key for the lights it placed: x, y, z of each, in turn.
pub const LIGHTS: &str = "lights";

pub fn decode(nbt: &Compound) -> Vec<[i32; 3]> {
    match nbt.get(&LIGHTS.into()) {
        Some(Tag::IntArray(v)) => v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        _ => Vec::new(),
    }
}

pub fn encode(lights: &[[i32; 3]]) -> Tag {
    Tag::IntArray(lights.iter().flatten().copied().collect())
}

/// Where the lantern at `origin` tries a light, as Torchmaster's lantern does: a random block in
/// the cube `radius` about it (`roll(n)` is a draw from `0..n`), no higher than 4 above the
/// column's surface (`surface`, the first free block over it, when known); none outside the
/// world's heights `min_y..max_y`.
pub fn target(origin: [i32; 3], radius: i32, roll: &mut impl FnMut(i32) -> i32, surface: impl FnOnce(i32, i32) -> Option<i32>, min_y: i32, max_y: i32) -> Option<[i32; 3]> {
    let diameter = radius * 2;
    let x = origin[0] + roll(diameter) - radius;
    let mut y = origin[1] + roll(diameter) - radius;
    let z = origin[2] + roll(diameter) - radius;
    if let Some(top) = surface(x, z) {
        y = y.min(top + 4);
    }
    (min_y..max_y).contains(&y).then_some([x, y, z])
}

/// A small, fast random stream (xorshift64*), seeded once from the clock: which block a lantern
/// tries next needs no more than that.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// A draw from `0..n` (0 when `n` is 0 or less).
    pub fn below(&mut self, n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 33) % n as u64) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lights_survive_a_round_trip() {
        let lights = vec![[1, 2, 3], [-40, -64, 1000]];
        let mut nbt = Compound::new();
        nbt.insert(LIGHTS.into(), encode(&lights));
        assert_eq!(decode(&nbt), lights);
        assert_eq!(decode(&Compound::new()), Vec::<[i32; 3]>::new());
    }

    #[test]
    fn targets_stay_in_the_cube_and_under_the_sky_limit() {
        let mut rng = Rng::new(7);
        for _ in 0..2000 {
            let t = target([0, 60, 0], 16, &mut |n| rng.below(n), |_, _| Some(64), -64, 320).unwrap();
            assert!((-16..16).contains(&t[0]) && (-16..16).contains(&t[2]));
            assert!((44..=68).contains(&t[1]), "{t:?}");
        }
    }

    #[test]
    fn a_target_outside_the_world_is_none() {
        let mut low = |_| 0;
        assert_eq!(target([0, -60, 0], 16, &mut low, |_, _| None, -64, 320), None);
        assert_eq!(target([0, -40, 0], 16, &mut low, |_, _| None, -64, 320), Some([-16, -56, -16]));
    }

    #[test]
    fn draws_cover_their_range() {
        let mut rng = Rng::new(12345);
        let mut seen = [false; 32];
        for _ in 0..5000 {
            seen[rng.below(32) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(rng.below(0), 0);
    }
}
