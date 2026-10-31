//! Rectangular mechanical crafting over real member inventories. The host
//! supplies matched results/remainders; this module reserves the whole commit.
use super::{INPUT, MachineState, OUTPUT, Stack, TickResult, surface};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub type Pos = [i32; 3];
#[derive(Clone, Debug)]
pub struct Grid {
    pub cells: Vec<Pos>,
    pub width: usize,
    pub height: usize,
}
impl Grid {
    pub fn identity(&self) -> u64 {
        self.cells
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .fold(0xcbf29ce484222325, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            })
    }
}
/// Faces north/south/east/west are 0/1/2/3. Rows run top to bottom and
/// columns left to right when looking directly at the front of the wall.
pub fn grids(nodes: &BTreeMap<Pos, u8>) -> Vec<Grid> {
    let mut remaining: BTreeSet<_> = nodes.keys().copied().collect();
    let mut result = Vec::new();
    while let Some(&start) = remaining.first() {
        remaining.remove(&start);
        let facing = nodes[&start];
        if facing > 3 {
            continue;
        }
        let right = match facing {
            0 => [-1, 0, 0],
            1 => [1, 0, 0],
            2 => [0, 0, -1],
            _ => [0, 0, 1],
        };
        let mut queue = VecDeque::from([start]);
        let mut cells = Vec::new();
        while let Some(p) = queue.pop_front() {
            cells.push(p);
            for d in [right, right.map(|v| -v), [0, 1, 0], [0, -1, 0]] {
                let q = p
                    .into_iter()
                    .zip(d)
                    .map(|(a, b)| a.checked_add(b))
                    .collect::<Option<Vec<_>>>();
                let Some(q) = q else { continue };
                let q = [q[0], q[1], q[2]];
                if nodes.get(&q) == Some(&facing) && remaining.remove(&q) {
                    queue.push_back(q);
                }
            }
        }
        if cells.len() > 81 {
            continue;
        }
        let column =
            |p: &Pos| i64::from(p[0]) * i64::from(right[0]) + i64::from(p[2]) * i64::from(right[2]);
        let left = cells.iter().map(column).min().unwrap();
        let right = cells.iter().map(column).max().unwrap();
        let top = cells.iter().map(|p| p[1]).max().unwrap();
        let bottom = cells.iter().map(|p| p[1]).min().unwrap();
        let w = right - left + 1;
        let h = i64::from(top) - i64::from(bottom) + 1;
        if !(1..=9).contains(&w) || !(1..=9).contains(&h) || w * h != cells.len() as i64 {
            continue;
        }
        cells.sort_by_key(|p| (std::cmp::Reverse(p[1]), column(p)));
        result.push(Grid {
            cells,
            width: w as usize,
            height: h as usize,
        });
    }
    result
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub material: u64,
    pub counts: Vec<u32>,
}
pub fn selected(slots: &[Option<Stack>]) -> Option<(usize, &Stack)> {
    INPUT.into_iter().find_map(|i| {
        slots
            .get(i)?
            .as_ref()
            .filter(|s| s.count > 0)
            .map(|s| (i, s))
    })
}
#[derive(Clone, Debug)]
pub struct Update {
    pub state: MachineState,
    pub after: Vec<Vec<Option<Stack>>>,
    pub result: TickResult,
}
#[allow(clippy::too_many_arguments)]
pub fn work(
    state: &MachineState,
    before: &[Vec<Option<Stack>>],
    identity: u64,
    recipe: &str,
    output: &Stack,
    remainders: &[Option<Stack>],
    rpm: f32,
    limit: impl Fn(&str) -> u32,
) -> Option<Update> {
    if before.is_empty()
        || before.len() > 81
        || before.len() != remainders.len()
        || before.iter().any(|s| s.len() < 18)
        || before.iter().all(|s| selected(s).is_none())
        || !create_simulation::valid_id(recipe)
        || output.count == 0
    {
        return None;
    }
    let mut material = identity;
    let mut counts = Vec::new();
    for slots in before {
        let selection = selected(slots);
        material = (material
            ^ selection.map_or(0, |(i, s)| surface::material(s).wrapping_add(i as u64 + 1)))
        .wrapping_mul(0x100000001b3);
        counts.push(selection.map_or(0, |(_, s)| s.count));
    }
    let binding = Binding { material, counts };
    let mut next = state.clone();
    let compatible = state.craft_binding.as_ref().is_some_and(|b| {
        b.material == binding.material
            && b.counts.len() == binding.counts.len()
            && b.counts.iter().zip(&binding.counts).all(|(a, b)| b >= a)
    });
    if !compatible || state.recipe.as_deref() != Some(recipe) {
        next.progress = 0.0;
    }
    next.recipe = Some(recipe.into());
    next.craft_binding = Some(binding);
    next.world_target = None;
    next.block_target = None;
    let mut after = before.to_vec();
    if !rpm.is_finite() || rpm == 0.0 {
        next.status = "NoPower".into();
        return Some(Update {
            state: next,
            after,
            result: TickResult::NoPower,
        });
    }
    next.progress = (next.progress + (rpm.abs() / 8.0).clamp(0.125, 32.0)).min(100.0);
    if next.progress < 100.0 {
        next.status = "Crafting".into();
        return Some(Update {
            state: next,
            after,
            result: TickResult::Working,
        });
    }
    for (slots, remainder) in after.iter_mut().zip(remainders) {
        if let Some((i, _)) = selected(slots) {
            let s = slots[i].as_mut()?;
            s.count -= 1;
            if s.count == 0 {
                slots[i] = None;
            }
        }
        if let Some(rest) = remainder
            && !super::insert(&mut slots[INPUT], rest, &limit)
        {
            next.status = "OutputBlocked".into();
            return Some(Update {
                state: next,
                after: before.to_vec(),
                result: TickResult::OutputBlocked,
            });
        }
    }
    if !super::insert(&mut after[0][OUTPUT], output, limit) {
        next.status = "OutputBlocked".into();
        return Some(Update {
            state: next,
            after: before.to_vec(),
            result: TickResult::OutputBlocked,
        });
    }
    next.progress = 0.0;
    next.craft_binding = None;
    next.status = "Completed".into();
    Some(Update {
        state: next,
        after,
        result: TickResult::Completed,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stack(id: &str, n: u32) -> Stack {
        Stack {
            id: id.into(),
            count: n,
            components: String::new(),
        }
    }
    fn members() -> Vec<Vec<Option<Stack>>> {
        let mut v = vec![vec![None; 27]; 2];
        v[0][0] = Some(stack("minecraft:oak_planks", 3));
        v[1][0] = Some(stack("minecraft:oak_planks", 3));
        v
    }
    #[test]
    fn grids_are_oriented_rectangles_and_reject_holes_and_oversize() {
        let m = BTreeMap::from([
            ([0, 1, 0], 0),
            ([1, 1, 0], 0),
            ([0, 0, 0], 0),
            ([1, 0, 0], 0),
        ]);
        let g = grids(&m).remove(0);
        assert_eq!(g.cells, vec![[1, 1, 0], [0, 1, 0], [1, 0, 0], [0, 0, 0]]);
        assert_eq!((g.width, g.height), (2, 2));
        let mut hole = m;
        hole.remove(&[0, 0, 0]);
        assert!(grids(&hole).is_empty());
        assert!(grids(&(0..10).map(|x| ([x, 0, 0], 0)).collect()).is_empty());
    }
    #[test]
    fn facing_and_depth_separate_groups() {
        let m = BTreeMap::from([([0, 0, 0], 0), ([0, 0, 1], 0), ([1, 0, 0], 1)]);
        assert_eq!(grids(&m).len(), 3);
    }
    #[test]
    fn all_inputs_and_one_output_commit_together() {
        let before = members();
        let mut state = MachineState::default();
        for _ in 0..50 {
            let u = work(
                &state,
                &before,
                1,
                "minecraft:stick",
                &stack("minecraft:stick", 4),
                &[None, None],
                32.0,
                |_| 64,
            )
            .unwrap();
            state = MachineState::decode(&u.state.encode()).unwrap();
            if u.result == TickResult::Completed {
                assert_eq!(u.after[0][0].as_ref().unwrap().count, 2);
                assert_eq!(u.after[1][0].as_ref().unwrap().count, 2);
                assert_eq!(u.after[0][9].as_ref().unwrap().count, 4);
                return;
            }
        }
        panic!("grid never completed");
    }
    #[test]
    fn full_outputs_rollback_every_ingredient() {
        let mut before = members();
        for i in OUTPUT {
            before[0][i] = Some(stack("minecraft:dirt", 64));
        }
        let mut state = MachineState::default();
        for _ in 0..30 {
            let u = work(
                &state,
                &before,
                1,
                "minecraft:stick",
                &stack("minecraft:stick", 4),
                &[None, None],
                32.0,
                |_| 64,
            )
            .unwrap();
            state = u.state;
            if u.result == TickResult::OutputBlocked {
                assert_eq!(u.after, before);
                return;
            }
        }
        panic!("did not block");
    }
    #[test]
    fn bucket_remainders_and_components_are_retained() {
        let mut before = vec![vec![None; 27]];
        before[0][0] = Some(stack("minecraft:water_bucket", 1));
        let rest = Stack {
            id: "minecraft:bucket".into(),
            count: 1,
            components: "bucket-name".into(),
        };
        let mut state = MachineState::default();
        for _ in 0..30 {
            let u = work(
                &state,
                &before,
                1,
                "minecraft:cake",
                &stack("minecraft:cake", 1),
                &[Some(rest.clone())],
                32.0,
                |_| 64,
            )
            .unwrap();
            state = u.state;
            if u.result == TickResult::Completed {
                assert_eq!(u.after[0][0], Some(rest));
                return;
            }
        }
        panic!("never completed");
    }
    #[test]
    fn growth_does_not_starve_work_but_removal_or_replacement_resets_it() {
        let mut before = members();
        let state = work(
            &MachineState::default(),
            &before,
            1,
            "minecraft:stick",
            &stack("minecraft:stick", 4),
            &[None, None],
            32.0,
            |_| 64,
        )
        .unwrap()
        .state;
        before[0][0].as_mut().unwrap().count += 1;
        assert_eq!(
            work(
                &state,
                &before,
                1,
                "minecraft:stick",
                &stack("minecraft:stick", 4),
                &[None, None],
                32.0,
                |_| 64
            )
            .unwrap()
            .state
            .progress,
            8.0
        );
        before[1][0].as_mut().unwrap().count -= 1;
        assert_eq!(
            work(
                &state,
                &before,
                1,
                "minecraft:stick",
                &stack("minecraft:stick", 4),
                &[None, None],
                32.0,
                |_| 64
            )
            .unwrap()
            .state
            .progress,
            4.0
        );
    }
}
