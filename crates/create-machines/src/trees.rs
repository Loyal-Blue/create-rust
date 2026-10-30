//! Bounded stationary saw planning over snapshots of loaded world cells.
use std::collections::{BTreeSet, VecDeque};
pub type Pos = [i32; 3];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    NotLog,
    NoLeaves,
    Unloaded,
    TooLarge,
}
pub fn family(name: &str) -> Option<&str> {
    let wood = name.strip_prefix("minecraft:")?.strip_suffix("_log")?;
    matches!(
        wood,
        "oak"
            | "spruce"
            | "birch"
            | "jungle"
            | "acacia"
            | "dark_oak"
            | "mangrove"
            | "cherry"
            | "pale_oak"
    )
    .then_some(wood)
}
/// Refuse oversized/incomplete trees before returning any destructive work.
/// Only same-family logs at/above the cut and within six blocks horizontally
/// and 32 vertically are eligible. Ordinary engine breaking supplies the drops.
pub fn plan(
    origin: Pos,
    mut block: impl FnMut(Pos) -> Option<String>,
) -> Result<Vec<Pos>, TreeError> {
    let first = block(origin).ok_or(TreeError::Unloaded)?;
    let wood = family(&first).ok_or(TreeError::NotLog)?;
    let leaves = format!("minecraft:{wood}_leaves");
    let mut logs = BTreeSet::from([origin]);
    let mut seen = BTreeSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    let mut has_leaves = false;
    while let Some(p) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let q = [
                        p[0].checked_add(dx).ok_or(TreeError::TooLarge)?,
                        p[1].checked_add(dy).ok_or(TreeError::TooLarge)?,
                        p[2].checked_add(dz).ok_or(TreeError::TooLarge)?,
                    ];
                    if q[1] < origin[1] || !seen.insert(q) {
                        continue;
                    }
                    if seen.len() > 4096 {
                        return Err(TreeError::TooLarge);
                    }
                    let name = block(q).ok_or(TreeError::Unloaded)?;
                    if name == leaves {
                        has_leaves = true;
                    }
                    if family(&name) != Some(wood) {
                        continue;
                    }
                    if (i64::from(q[0]) - i64::from(origin[0])).abs() > 6
                        || (i64::from(q[2]) - i64::from(origin[2])).abs() > 6
                        || i64::from(q[1]) - i64::from(origin[1]) > 32
                        || logs.len() >= 128
                    {
                        return Err(TreeError::TooLarge);
                    }
                    logs.insert(q);
                    queue.push_back(q);
                }
            }
        }
    }
    if !has_leaves {
        return Err(TreeError::NoLeaves);
    }
    let mut result: Vec<_> = logs.into_iter().collect();
    result.sort_by_key(|p| (std::cmp::Reverse(p[1]), p[0], p[2]));
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn world() -> BTreeMap<Pos, String> {
        BTreeMap::from([
            ([0, 0, 0], "minecraft:oak_log".into()),
            ([0, 1, 0], "minecraft:oak_log".into()),
            ([1, 2, 0], "minecraft:oak_log".into()),
            ([2, 2, 0], "minecraft:oak_leaves".into()),
        ])
    }
    fn read(map: &BTreeMap<Pos, String>, p: Pos) -> Option<String> {
        Some(
            map.get(&p)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
    #[test]
    fn diagonal_branches_fall_top_down_and_roots_remain() {
        let mut m = world();
        m.insert([0, -1, 0], "minecraft:oak_log".into());
        let p = plan([0, 0, 0], |p| read(&m, p)).unwrap();
        assert_eq!(p, vec![[1, 2, 0], [0, 1, 0], [0, 0, 0]]);
    }
    #[test]
    fn bare_pillars_and_stripped_logs_do_not_form_trees() {
        let mut m = world();
        m.remove(&[2, 2, 0]);
        assert_eq!(plan([0, 0, 0], |p| read(&m, p)), Err(TreeError::NoLeaves));
        m.insert([0, 0, 0], "minecraft:stripped_oak_log".into());
        assert_eq!(plan([0, 0, 0], |p| read(&m, p)), Err(TreeError::NotLog));
    }
    #[test]
    fn unloaded_neighbors_abort_the_whole_plan() {
        let m = world();
        assert_eq!(
            plan([0, 0, 0], |p| if p[0] < 0 { None } else { read(&m, p) }),
            Err(TreeError::Unloaded)
        );
    }
    #[test]
    fn large_trees_abort_instead_of_returning_a_partial_cut() {
        let mut m = world();
        for x in 0..=7 {
            m.insert([x, 2, 0], "minecraft:oak_log".into());
        }
        assert_eq!(plan([0, 0, 0], |p| read(&m, p)), Err(TreeError::TooLarge));
        let mut m = world();
        for y in 0..=33 {
            m.insert([0, y, 0], "minecraft:oak_log".into());
        }
        assert_eq!(plan([0, 0, 0], |p| read(&m, p)), Err(TreeError::TooLarge));
    }
    #[test]
    fn other_species_and_plank_builds_are_excluded() {
        let mut m = world();
        m.insert([-1, 0, 0], "minecraft:birch_log".into());
        m.insert([0, 0, 1], "minecraft:oak_planks".into());
        assert_eq!(plan([0, 0, 0], |p| read(&m, p)).unwrap().len(), 3);
    }
    #[test]
    fn coordinate_overflow_is_rejected() {
        assert_eq!(
            plan([i32::MAX, 0, 0], |_| Some("minecraft:oak_log".into())),
            Err(TreeError::TooLarge)
        );
    }
}
