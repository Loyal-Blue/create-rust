//! Shared square tank volumes. Only per-block fluid is persisted; aggregate
//! views are temporary transaction inputs and must be scattered before saving.
use super::{MachineState, TANK_CAPACITY};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub type Pos = [i32; 3];
pub const MAX_BLOCKS: usize = 3 * 3 * 32;
#[derive(Clone, Debug)]
pub struct Group {
    pub members: Vec<Pos>,
}
impl Group {
    pub fn controller(&self) -> Pos {
        self.members[0]
    }
    pub fn capacity(&self) -> u32 {
        self.members.len() as u32 * TANK_CAPACITY
    }
    pub fn gather(&self, states: &BTreeMap<Pos, MachineState>) -> Option<MachineState> {
        if self.members.is_empty() || self.members.len() > MAX_BLOCKS {
            return None;
        }
        let mut view = states.get(&self.controller())?.clone();
        view.fluid_in.clear();
        view.fluid_out.clear();
        let mut total = 0u64;
        for p in &self.members {
            let s = states.get(p)?;
            let member: u64 = s
                .fluid_in
                .values()
                .chain(s.fluid_out.values())
                .map(|n| u64::from(*n))
                .sum();
            // Legacy overfull/mixed stores stay independent and drainable.
            if member > u64::from(TANK_CAPACITY) {
                return None;
            }
            for (id, n) in s.fluid_in.iter().chain(&s.fluid_out) {
                if *n == 0 {
                    continue;
                }
                if !view.fluid_in.is_empty() && !view.fluid_in.contains_key(id) {
                    return None;
                }
                *view.fluid_in.entry(id.clone()).or_default() += n;
                total += u64::from(*n);
            }
        }
        (total <= u64::from(self.capacity())).then_some(view)
    }
    pub fn scatter(
        &self,
        view: &MachineState,
        originals: &BTreeMap<Pos, MachineState>,
    ) -> Option<BTreeMap<Pos, MachineState>> {
        self.gather(originals)?;
        let mut fluid = None;
        let mut total = 0u64;
        for (id, n) in view.fluid_in.iter().chain(&view.fluid_out) {
            if *n == 0 {
                continue;
            }
            if fluid.is_some_and(|f| f != id.as_str()) {
                return None;
            }
            fluid = Some(id.as_str());
            total += u64::from(*n);
        }
        if total > u64::from(self.capacity()) {
            return None;
        }
        let mut order = self.members.clone();
        order.sort_by_key(|p| (p[1], p[0], p[2]));
        let mut result = BTreeMap::new();
        for p in order {
            let mut s = originals.get(&p)?.clone();
            s.fluid_in.clear();
            s.fluid_out.clear();
            let n = total.min(u64::from(TANK_CAPACITY)) as u32;
            if n > 0 {
                s.fluid_in.insert(fluid?.into(), n);
            }
            total -= u64::from(n);
            result.insert(p, s);
        }
        Some(result)
    }
}
fn neighbors(p: Pos) -> impl Iterator<Item = Pos> {
    [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .into_iter()
    .filter_map(move |d| {
        Some([
            p[0].checked_add(d[0])?,
            p[1].checked_add(d[1])?,
            p[2].checked_add(d[2])?,
        ])
    })
}
/// Accept complete 1x1, 2x2 or 3x3 footprints, up to 32 layers high.
/// Irregular/touching oversized components remain separate stores.
pub fn groups(tanks: &BTreeSet<Pos>) -> Vec<Group> {
    let mut remaining = tanks.clone();
    let mut result = Vec::new();
    while let Some(&start) = remaining.first() {
        remaining.remove(&start);
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        let mut min = start;
        let mut max = start;
        while let Some(p) = queue.pop_front() {
            // Traverse the entire component so no bounded subset forms a fake tank.
            component.push(p);
            for a in 0..3 {
                min[a] = min[a].min(p[a]);
                max[a] = max[a].max(p[a]);
            }
            for q in neighbors(p) {
                if remaining.remove(&q) {
                    queue.push_back(q);
                }
            }
        }
        let dx = i64::from(max[0]) - i64::from(min[0]) + 1;
        let dy = i64::from(max[1]) - i64::from(min[1]) + 1;
        let dz = i64::from(max[2]) - i64::from(min[2]) + 1;
        if dx == dz
            && (1..=3).contains(&dx)
            && (1..=32).contains(&dy)
            && component.len() <= MAX_BLOCKS
            && dx * dy * dz == component.len() as i64
        {
            component.sort();
            result.push(Group { members: component });
        }
    }
    result
}
pub fn fill(view: &mut MachineState, capacity: u32, id: &str, amount: u32) -> bool {
    if capacity == 0
        || capacity > MAX_BLOCKS as u32 * TANK_CAPACITY
        || amount == 0
        || !create_simulation::valid_id(id)
    {
        return false;
    }
    let total: u64 = view
        .fluid_in
        .values()
        .chain(view.fluid_out.values())
        .map(|n| u64::from(*n))
        .sum();
    if total + u64::from(amount) > u64::from(capacity)
        || view
            .fluid_in
            .iter()
            .chain(&view.fluid_out)
            .any(|(f, n)| *n > 0 && f != id)
    {
        return false;
    }
    *view.fluid_in.entry(id.into()).or_default() += amount;
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    fn states(g: &Group) -> BTreeMap<Pos, MachineState> {
        g.members
            .iter()
            .map(|p| (*p, MachineState::default()))
            .collect()
    }
    #[test]
    fn complete_square_columns_form_and_irregular_shapes_do_not() {
        let column = BTreeSet::from([[0, 0, 0], [0, 1, 0], [0, 2, 0]]);
        let g = groups(&column);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].capacity(), 24000);
        let square: BTreeSet<_> = (0..2)
            .flat_map(|x| (0..2).flat_map(move |y| (0..2).map(move |z| [x, y, z])))
            .collect();
        assert_eq!(groups(&square)[0].capacity(), 64000);
        assert!(groups(&BTreeSet::from([[0, 0, 0], [1, 0, 0], [0, 0, 1]])).is_empty());
    }
    #[test]
    fn oversized_components_never_form_partial_groups() {
        let high = (0..33).map(|y| [0, y, 0]).collect();
        assert!(groups(&high).is_empty());
        let wide = (0..4)
            .flat_map(|x| (0..4).map(move |z| [x, 0, z]))
            .collect();
        assert!(groups(&wide).is_empty());
    }
    #[test]
    fn gather_scatter_saves_only_per_block_volumes_and_preserves_metadata() {
        let g = groups(&BTreeSet::from([[0, 0, 0], [0, 1, 0], [0, 2, 0]])).remove(0);
        let mut original = states(&g);
        original.get_mut(&[0, 2, 0]).unwrap().direction = -1;
        let mut view = g.gather(&original).unwrap();
        assert!(fill(&mut view, g.capacity(), "minecraft:water", 19000));
        let saved = g.scatter(&view, &original).unwrap();
        assert_eq!(saved[&[0, 0, 0]].fluid_in["minecraft:water"], 8000);
        assert_eq!(saved[&[0, 1, 0]].fluid_in["minecraft:water"], 8000);
        assert_eq!(saved[&[0, 2, 0]].fluid_in["minecraft:water"], 3000);
        assert_eq!(saved[&[0, 2, 0]].direction, -1);
        for state in saved.values() {
            assert_eq!(MachineState::decode(&state.encode()).unwrap(), *state);
        }
        assert_eq!(g.gather(&saved).unwrap().fluid_in["minecraft:water"], 19000);
    }
    #[test]
    fn mixed_or_overfull_legacy_data_remain_unmerged() {
        let g = groups(&BTreeSet::from([[0, 0, 0], [0, 1, 0]])).remove(0);
        let mut original = states(&g);
        original
            .get_mut(&[0, 0, 0])
            .unwrap()
            .insert_fluid("minecraft:water", 1000);
        original
            .get_mut(&[0, 1, 0])
            .unwrap()
            .insert_fluid("minecraft:lava", 1000);
        assert!(g.gather(&original).is_none());
        original.get_mut(&[0, 1, 0]).unwrap().fluid_in.clear();
        original
            .get_mut(&[0, 0, 0])
            .unwrap()
            .fluid_out
            .insert("minecraft:water".into(), 8000);
        assert!(g.gather(&original).is_none());
    }
    #[test]
    fn shared_capacity_and_bucket_extraction_are_atomic() {
        let g = groups(&BTreeSet::from([[0, 0, 0], [0, 1, 0]])).remove(0);
        let original = states(&g);
        let mut view = g.gather(&original).unwrap();
        assert!(fill(&mut view, g.capacity(), "minecraft:water", 16000));
        let before = view.clone();
        assert!(!fill(&mut view, g.capacity(), "minecraft:water", 1));
        assert!(!fill(&mut view, g.capacity(), "minecraft:lava", 1));
        assert_eq!(view, before);
        assert!(view.extract_fluid("minecraft:water", 1000));
        let saved = g.scatter(&view, &original).unwrap();
        assert_eq!(g.gather(&saved).unwrap().fluid_in["minecraft:water"], 15000);
    }
    #[test]
    fn missing_members_or_oversized_views_never_commit() {
        let g = groups(&BTreeSet::from([[0, 0, 0], [0, 1, 0]])).remove(0);
        let mut original = states(&g);
        let mut view = g.gather(&original).unwrap();
        view.fluid_in.insert("minecraft:water".into(), 16001);
        assert!(g.scatter(&view, &original).is_none());
        original.remove(&[0, 1, 0]);
        assert!(g.gather(&original).is_none());
    }
}
