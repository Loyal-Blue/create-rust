//! Deterministic prototype zinc veins. The host applies these only to fresh
//! overworld chunks and only replaces natural stone/deepslate.
pub fn zinc_positions(seed: i64, cx: i32, cz: i32, min_y: i32, max_y: i32) -> Vec<[i32; 3]> {
    if max_y <= min_y {
        return vec![];
    }
    let mut rng = (seed as u64)
        ^ (cx as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cz as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    let mut next = || {
        rng = rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = rng;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        n ^ (n >> 31)
    };
    let mut out = std::collections::BTreeSet::new();
    for _ in 0..8 {
        let mut p = [
            (next() % 16) as i32,
            (-40 + (next() % 96) as i32).clamp(min_y, max_y - 1),
            (next() % 16) as i32,
        ];
        for _ in 0..6 {
            out.insert([cx * 16 + p[0], p[1], cz * 16 + p[2]]);
            let a = (next() % 3) as usize;
            let d = if next() & 1 == 0 { -1 } else { 1 };
            p[a] = (p[a] + d).clamp(
                if a == 1 { min_y } else { 0 },
                if a == 1 { max_y - 1 } else { 15 },
            );
        }
    }
    out.into_iter().collect()
}
pub fn should_generate(fresh: bool, overworld: bool, already_generated: bool) -> bool {
    fresh && overworld && !already_generated
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_repeatable_veins() {
        let a = zinc_positions(42, -3, 5, -64, 320);
        assert_eq!(a, zinc_positions(42, -3, 5, -64, 320));
        assert!(!a.is_empty());
        assert!(
            a.iter()
                .all(|p| p[0] >> 4 == -3 && p[2] >> 4 == 5 && (-64..320).contains(&p[1]))
        );
        assert_ne!(a, zinc_positions(43, -3, 5, -64, 320));
    }
    #[test]
    fn never_retro_generates_existing_chunks() {
        assert!(!should_generate(false, true, false));
        assert!(!should_generate(true, false, false));
        assert!(!should_generate(true, true, true));
        assert!(should_generate(true, true, false));
    }
}
