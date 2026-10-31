//! Bounded deployer placement plans; the host's block setter decides acceptance.
use super::{INPUT, InventoryPlan, MachineState, Stack, TickResult, surface};
pub const RECIPE: &str = "create:world/block_placement";
pub fn target(origin: [i32; 3], direction: [i32; 3]) -> Option<[i32; 3]> {
    if direction.iter().any(|v| !(-1..=1).contains(v))
        || direction.iter().map(|v| v.abs()).sum::<i32>() != 1
    {
        return None;
    }
    Some([
        origin[0].checked_add(direction[0] * 2)?,
        origin[1].checked_add(direction[1] * 2)?,
        origin[2].checked_add(direction[2] * 2)?,
    ])
}
/// Plain stable building cubes only; never erase component data or place
/// inventory blocks, tools, fluids, falling blocks or directional devices.
pub fn building_block(stack: &Stack) -> bool {
    if stack.count == 0 || !stack.components.is_empty() {
        return false;
    }
    let Some(id) = stack.id.strip_prefix("minecraft:") else {
        return false;
    };
    if matches!(
        id,
        "stone"
            | "cobblestone"
            | "mossy_cobblestone"
            | "dirt"
            | "coarse_dirt"
            | "rooted_dirt"
            | "grass_block"
            | "deepslate"
            | "cobbled_deepslate"
            | "andesite"
            | "diorite"
            | "granite"
            | "tuff"
            | "calcite"
            | "netherrack"
            | "end_stone"
            | "bricks"
            | "stone_bricks"
            | "glass"
            | "obsidian"
            | "terracotta"
    ) {
        return true;
    }
    let wood = id.strip_prefix("stripped_").unwrap_or(id);
    ["_planks", "_log", "_wood", "_stem", "_hyphae"]
        .iter()
        .any(|suffix| {
            wood.strip_suffix(suffix).is_some_and(|family| {
                matches!(
                    family,
                    "oak"
                        | "spruce"
                        | "birch"
                        | "jungle"
                        | "acacia"
                        | "dark_oak"
                        | "mangrove"
                        | "cherry"
                        | "pale_oak"
                        | "crimson"
                        | "warped"
                        | "bamboo"
                )
            })
        })
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub state: MachineState,
    pub inventory: InventoryPlan,
    pub item: String,
    pub result: TickResult,
}
pub fn plan(
    state: &MachineState,
    slots: &[Option<Stack>],
    at: [i32; 3],
    rpm: f32,
) -> Option<Placement> {
    if slots.len() < 18 {
        return None;
    }
    let index = INPUT
        .into_iter()
        .find(|i| slots[*i].as_ref().is_some_and(building_block))?;
    let input = slots[index].as_ref()?;
    let mut next = state.clone();
    let compatible = next.world_target.is_some_and(|b| {
        b.entity == index as u32 && b.material == surface::material(input) && input.count >= b.count
    });
    if next.recipe.as_deref() != Some(RECIPE) || next.block_target != Some(at) || !compatible {
        next.progress = 0.0;
    }
    next.recipe = Some(RECIPE.into());
    next.block_target = Some(at);
    let mut after = slots.to_vec();
    let result = if !rpm.is_finite() || rpm == 0.0 {
        next.status = "NoPower".into();
        TickResult::NoPower
    } else {
        next.progress = (next.progress + (rpm.abs() / 32.0).clamp(0.03125, 8.0)).min(20.0);
        if next.progress < 20.0 {
            next.status = "Placing".into();
            TickResult::Working
        } else {
            let stack = after[index].as_mut()?;
            stack.count -= 1;
            if stack.count == 0 {
                after[index] = None;
            }
            next.progress = 0.0;
            next.status = "Placed".into();
            TickResult::Completed
        }
    };
    next.world_target = after[index].as_ref().map(|s| surface::Binding {
        entity: index as u32,
        fingerprint: surface::fingerprint(s),
        material: surface::material(s),
        count: s.count,
    });
    Some(Placement {
        state: next,
        inventory: InventoryPlan {
            before: slots.to_vec(),
            after,
        },
        item: input.id.clone(),
        result,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn slots(id: &str) -> Vec<Option<Stack>> {
        let mut v = vec![None; 27];
        v[0] = Some(Stack {
            id: id.into(),
            count: 3,
            components: String::new(),
        });
        v
    }
    #[test]
    fn targets_are_cardinal_and_checked_for_overflow() {
        assert_eq!(target([0, 2, 0], [0, -1, 0]), Some([0, 0, 0]));
        assert_eq!(target([0, 0, 0], [1, 0, 0]), Some([2, 0, 0]));
        assert!(target([i32::MAX, 0, 0], [1, 0, 0]).is_none());
        assert!(target([0, 0, 0], [1, 1, 0]).is_none());
        assert!(target([0, 0, 0], [i32::MIN, 0, 0]).is_none());
    }
    #[test]
    fn named_items_and_unsafe_blocks_are_refused() {
        for id in [
            "minecraft:chest",
            "minecraft:water_bucket",
            "minecraft:sand",
            "minecraft:tnt",
            "minecraft:oak_slab",
            "create:deployer",
        ] {
            assert!(plan(&MachineState::default(), &slots(id), [2, 0, 0], 32.0).is_none());
        }
        let mut named = slots("minecraft:stone");
        named[0].as_mut().unwrap().components = "custom-name".into();
        assert!(plan(&MachineState::default(), &named, [2, 0, 0], 32.0).is_none());
    }
    #[test]
    fn slow_drive_completes_once_and_consumes_one_item() {
        let slots = slots("minecraft:oak_planks");
        let mut state = MachineState::default();
        for tick in 0..100 {
            let p = plan(&state, &slots, [2, 0, 0], 8.0).unwrap();
            state = MachineState::decode(&p.state.encode()).unwrap();
            if p.result == TickResult::Completed {
                assert_eq!(tick, 79);
                assert_eq!(p.inventory.after[0].as_ref().unwrap().count, 2);
                return;
            }
            assert_eq!(p.inventory.before, p.inventory.after);
        }
        panic!("deployer never finished");
    }
    #[test]
    fn changed_targets_and_materials_reset_work() {
        let slots = slots("minecraft:stone");
        let mut state = MachineState::default();
        for _ in 0..19 {
            state = plan(&state, &slots, [2, 0, 0], 32.0).unwrap().state;
        }
        let changed = plan(&state, &slots, [-2, 0, 0], 32.0).unwrap();
        assert_eq!(changed.state.progress, 1.0);
        let changed = plan(&state, &self::slots("minecraft:dirt"), [2, 0, 0], 32.0).unwrap();
        assert_eq!(changed.state.progress, 1.0);
        let mut fewer = slots;
        fewer[0].as_mut().unwrap().count = 2;
        assert_eq!(
            plan(&state, &fewer, [2, 0, 0], 32.0)
                .unwrap()
                .state
                .progress,
            1.0
        );
    }
    #[test]
    fn no_power_preserves_work_and_growing_streams_do_not_starve_it() {
        let mut slots = slots("minecraft:stone");
        let state = plan(&MachineState::default(), &slots, [2, 0, 0], 32.0)
            .unwrap()
            .state;
        let paused = plan(&state, &slots, [2, 0, 0], 0.0).unwrap();
        assert_eq!(paused.state.progress, 1.0);
        assert_eq!(paused.inventory.after, slots);
        slots[0].as_mut().unwrap().count = 4;
        assert_eq!(
            plan(&state, &slots, [2, 0, 0], 32.0)
                .unwrap()
                .state
                .progress,
            2.0
        );
    }
    #[test]
    fn changed_inventory_rejects_a_stale_commit() {
        let slots = slots("minecraft:stone");
        let mut state = MachineState::default();
        for _ in 0..20 {
            state = plan(&state, &slots, [2, 0, 0], 32.0).unwrap().state;
        }
        let p = plan(&state, &slots, [2, 0, 0], 32.0).unwrap();
        let mut changed = slots;
        changed[0].as_mut().unwrap().count = 2;
        assert!(!p.inventory.commit(&mut changed));
        assert_eq!(changed[0].as_ref().unwrap().count, 2);
    }
}
