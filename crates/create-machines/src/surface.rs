//! Processing a live world stack. Plans contain before/after values; persisted
//! machine state stores only a target fingerprint, never another item inventory.
use super::{Heat, INPUT, MachineState, OUTPUT, Stack, Stock, TANK_CAPACITY, TickResult};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub entity: u32,
    pub fingerprint: u64,
    pub material: u64,
    pub count: u32,
}
pub fn material(stack: &Stack) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for byte in stack.id.bytes().chain([0]).chain(stack.components.bytes()) {
        h = (h ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    h
}
pub fn fingerprint(stack: &Stack) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for byte in stack
        .id
        .bytes()
        .chain([0])
        .chain(stack.count.to_le_bytes())
        .chain([0])
        .chain(stack.components.bytes())
    {
        h = (h ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    h
}
pub fn recipe<'a>(
    state: &MachineState,
    stack: &Stack,
    station: &str,
    recipes: &'a [create_simulation::Recipe],
) -> Option<&'a create_simulation::Recipe> {
    if stack.count == 0 {
        return None;
    }
    let stock = Stock {
        items: std::collections::BTreeMap::from([(stack.id.clone(), stack.count)]),
        fluids: state.fluid_in.clone(),
        slots: usize::MAX,
        tank_capacity: TANK_CAPACITY,
    };
    create_simulation::matching_recipe(&stock, station, recipes)
}
#[derive(Clone, Debug)]
pub struct WorldUpdate {
    pub state: MachineState,
    pub before: Stack,
    pub remaining: Option<Stack>,
    pub outputs: Vec<Stack>,
    pub result: TickResult,
}
fn finish(
    state: MachineState,
    before: &Stack,
    slots: &[Option<Stack>],
    result: TickResult,
) -> WorldUpdate {
    WorldUpdate {
        state,
        before: before.clone(),
        remaining: slots[INPUT.start].clone(),
        outputs: slots[OUTPUT].iter().flatten().cloned().collect(),
        result,
    }
}
/// Presses and fan processes convert a whole compatible stack in one work cycle.
/// Filling/draining stay one operation per cycle so each operation reserves fluid.
/// All batch outputs must fit before the host can consume any live input.
#[allow(clippy::too_many_arguments)]
pub fn work(
    state: &MachineState,
    entity: u32,
    stack: &Stack,
    station: &str,
    rpm: f32,
    heat: Heat,
    recipes: &[create_simulation::Recipe],
    limit: impl Fn(&str) -> u32,
) -> Result<WorldUpdate, String> {
    let binding = Binding {
        entity,
        fingerprint: fingerprint(stack),
        material: material(stack),
        count: stack.count,
    };
    let mut base = state.clone();
    let compatible_growth = base.world_target.is_some_and(|old| {
        old.entity == entity && old.material == binding.material && stack.count >= old.count
    });
    if base.world_target != Some(binding) && !compatible_growth {
        base.progress = 0.0;
        base.recipe = None;
    }
    base.world_target = Some(binding);
    let mut slots = vec![None; 27];
    slots[INPUT.start] = Some(stack.clone());
    let original_recipe = recipe(&base, stack, station, recipes).map(|r| (r.id, r.work));
    let mut update =
        super::process_with_catalog(&base, &slots, station, rpm, heat, recipes, &limit)?;
    let bulk = matches!(
        station,
        "pressing" | "splashing" | "blasting" | "smoking" | "haunting" | "crushing"
    );
    if update.result == TickResult::Completed
        && bulk
        && let Some((id, work)) = original_recipe
    {
        for _ in 1..64 {
            let Some(remaining) = update.inventory.after[INPUT.start].as_ref() else {
                break;
            };
            if recipe(&update.state, remaining, station, recipes).is_none_or(|r| r.id != id) {
                break;
            }
            let mut ready = update.state.clone();
            ready.progress = work.max(1) as f32;
            let next = super::process_with_catalog(
                &ready,
                &update.inventory.after,
                station,
                rpm,
                heat,
                recipes,
                &limit,
            )?;
            if next.result == TickResult::OutputBlocked {
                base.progress = work.max(1) as f32;
                base.recipe = Some(id.into());
                base.status = "OutputBlocked".into();
                return Ok(finish(base, stack, &slots, TickResult::OutputBlocked));
            }
            if next.result != TickResult::Completed {
                break;
            }
            update = next;
        }
    }
    let mut next = update.state;
    next.world_target = update.inventory.after[INPUT.start]
        .as_ref()
        .map(|s| Binding {
            entity,
            fingerprint: fingerprint(s),
            material: material(s),
            count: s.count,
        });
    Ok(finish(next, stack, &update.inventory.after, update.result))
}
/// Bulk pressing for a depot's actual slots. Incoming identical materials may
/// extend a stack without resetting its work; outputs remain fully reserved.
#[allow(clippy::too_many_arguments)]
pub fn work_inventory(
    state: &MachineState,
    slots: &[Option<Stack>],
    station: &str,
    rpm: f32,
    heat: Heat,
    recipes: &[create_simulation::Recipe],
    limit: impl Fn(&str) -> u32,
) -> Result<super::Update, String> {
    if slots.len() < 18 {
        return Err("Depot needs input and output rows".into());
    }
    let index = INPUT.into_iter().find(|i| {
        slots[*i]
            .as_ref()
            .is_some_and(|s| recipe(state, s, station, recipes).is_some())
    });
    let Some(index) = index else {
        return super::process_with_catalog(state, slots, station, rpm, heat, recipes, limit);
    };
    let input = slots[index].as_ref().unwrap();
    let mut base = state.clone();
    if base
        .world_target
        .is_some_and(|b| b.entity == 0 && b.material == material(input))
    {
        base.world_target = Some(Binding {
            entity: 0,
            fingerprint: fingerprint(input),
            material: material(input),
            count: input.count,
        });
    }
    let update = work(&base, 0, input, station, rpm, heat, recipes, &limit)?;
    let mut after = slots.to_vec();
    after[index] = update.remaining;
    for output in &update.outputs {
        if !super::insert(&mut after[OUTPUT], output, &limit) {
            let mut blocked = base;
            blocked.progress = update
                .state
                .recipe
                .as_deref()
                .and_then(|id| recipes.iter().find(|r| r.id == id))
                .map_or(state.progress, |r| r.work.max(1) as f32);
            blocked.recipe = update.state.recipe;
            blocked.status = "OutputBlocked".into();
            blocked.world_target = Some(Binding {
                entity: 0,
                fingerprint: fingerprint(input),
                material: material(input),
                count: input.count,
            });
            return Ok(super::Update {
                state: blocked,
                inventory: super::InventoryPlan {
                    before: slots.to_vec(),
                    after: slots.to_vec(),
                },
                result: TickResult::OutputBlocked,
            });
        }
    }
    Ok(super::Update {
        state: update.state,
        inventory: super::InventoryPlan {
            before: slots.to_vec(),
            after,
        },
        result: update.result,
    })
}

pub fn below_controller(controller: [i32; 3], item: [f64; 3]) -> bool {
    (item[0] - (f64::from(controller[0]) + 0.5)).abs() <= 0.45
        && (item[2] - (f64::from(controller[2]) + 0.5)).abs() <= 0.45
        && {
            let down = f64::from(controller[1]) - item[1];
            (0.0..=2.4).contains(&down)
        }
}
pub fn in_stream(origin: [i32; 3], direction: [i32; 3], reach: u32, item: [f64; 3]) -> bool {
    if direction.iter().any(|v| !(-1..=1).contains(v))
        || reach == 0
        || direction.iter().map(|v| v.abs()).sum::<i32>() != 1
        || !item.iter().all(|v| v.is_finite())
    {
        return false;
    }
    let d = [
        item[0] - f64::from(origin[0]) - 0.5,
        item[1] + 0.125 - f64::from(origin[1]) - 0.5,
        item[2] - f64::from(origin[2]) - 0.5,
    ];
    let Some(axis) = direction.iter().position(|v| *v != 0) else {
        return false;
    };
    let along = d[axis] * f64::from(direction[axis]);
    (0.5..=f64::from(reach) + 0.5).contains(&along)
        && d.iter()
            .enumerate()
            .all(|(i, v)| i == axis || v.abs() <= 0.6)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn iron(count: u32) -> Stack {
        Stack {
            id: "minecraft:iron_ingot".into(),
            count,
            components: "{name:1}".into(),
        }
    }
    fn press(state: &MachineState, id: u32, s: &Stack, limit: impl Fn(&str) -> u32) -> WorldUpdate {
        work(
            state,
            id,
            s,
            "pressing",
            16.0,
            Heat::None,
            create_simulation::active_recipes(),
            limit,
        )
        .unwrap()
    }
    #[test]
    fn whole_named_stack_presses_in_one_cycle() {
        let input = iron(64);
        let mut state = MachineState::default();
        for tick in 1..=50 {
            let u = press(&state, 9, &input, |_| 64);
            state = MachineState::decode(&u.state.encode()).unwrap();
            if u.result == TickResult::Completed {
                assert_eq!(tick, 50);
                assert!(u.remaining.is_none());
                assert_eq!(u.outputs.len(), 1);
                assert_eq!(u.outputs[0].count, 64);
                assert_eq!(u.outputs[0].id, "create:iron_sheet");
                assert!(state.world_target.is_none());
                return;
            }
        }
        panic!("press did not finish");
    }
    #[test]
    fn new_entities_and_replaced_material_reset_but_merging_does_not_starve_work() {
        let input = iron(3);
        let mut state = MachineState::default();
        for _ in 0..20 {
            state = press(&state, 1, &input, |_| 64).state;
        }
        assert_eq!(state.progress, 40.0);
        assert_eq!(press(&state, 2, &input, |_| 64).state.progress, 2.0);
        assert_eq!(press(&state, 1, &iron(4), |_| 64).state.progress, 42.0);
        assert_eq!(press(&state, 1, &iron(2), |_| 64).state.progress, 2.0);
        let mut renamed = iron(3);
        renamed.components = "{name:2}".into();
        assert_eq!(press(&state, 1, &renamed, |_| 64).state.progress, 2.0);
    }
    #[test]
    fn blocked_bulk_output_preserves_every_input_and_rng() {
        let input = iron(64);
        let initial = MachineState::default();
        let mut state = initial.clone();
        for _ in 0..50 {
            let u = press(&state, 9, &input, |_| 1);
            state = u.state;
            if u.result == TickResult::OutputBlocked {
                assert_eq!(u.remaining, Some(input.clone()));
                assert!(u.outputs.is_empty());
                assert_eq!(state.rng, initial.rng);
                let retry = press(&state, 9, &input, |_| 64);
                assert_eq!(retry.result, TickResult::Completed);
                assert_eq!(retry.outputs[0].count, 64);
                return;
            }
        }
        panic!("backpressure was not reported");
    }
    #[test]
    fn filling_reserves_one_bottle_and_fluid_per_cycle() {
        let input = Stack {
            id: "minecraft:glass_bottle".into(),
            count: 2,
            components: String::new(),
        };
        let mut state = MachineState::default();
        assert!(state.insert_fluid("create:honey", 250));
        for _ in 0..40 {
            let u = work(
                &state,
                1,
                &input,
                "filling",
                32.0,
                Heat::None,
                create_simulation::active_recipes(),
                |_| 64,
            )
            .unwrap();
            state = u.state;
            if u.result == TickResult::Completed {
                assert_eq!(u.remaining.as_ref().unwrap().count, 1);
                assert_eq!(u.outputs[0].id, "minecraft:honey_bottle");
                assert!(state.fluid_in.is_empty());
                assert!(
                    recipe(
                        &state,
                        u.remaining.as_ref().unwrap(),
                        "filling",
                        create_simulation::active_recipes()
                    )
                    .is_none()
                );
                return;
            }
        }
        panic!("filling did not finish");
    }
    #[test]
    fn stopping_preserves_work_but_not_a_duplicate_inventory() {
        let input = iron(2);
        let state = press(&MachineState::default(), 1, &input, |_| 64).state;
        let u = work(
            &state,
            1,
            &input,
            "pressing",
            0.0,
            Heat::None,
            create_simulation::active_recipes(),
            |_| 64,
        )
        .unwrap();
        assert_eq!(u.result, TickResult::NoPower);
        assert_eq!(u.remaining, Some(input));
        assert_eq!(u.state.progress, state.progress);
        let json = u.state.encode();
        assert!(!json.contains("minecraft:iron_ingot"));
        assert!(!json.contains("{name:1}"));
    }
    #[test]
    fn surface_regions_are_bounded_and_directional() {
        assert!(below_controller([0, 2, 0], [0.5, 0.8125, 0.5]));
        assert!(!below_controller([0, 2, 0], [1.5, 0.8125, 0.5]));
        assert!(in_stream([0, 0, 0], [0, 0, 1], 8, [0.5, 0.4, 4.5]));
        assert!(!in_stream([0, 0, 0], [0, 0, 1], 8, [0.5, 0.4, -4.5]));
        assert!(!in_stream([0, 0, 0], [0, 0, 1], 8, [1.5, 0.4, 4.5]));
    }
}

#[cfg(test)]
mod depot_tests {
    use super::*;
    #[test]
    fn incoming_material_does_not_starve_a_depot_press() {
        let mut state = MachineState::default();
        let mut slots = vec![None; 27];
        slots[0] = Some(Stack {
            id: "minecraft:iron_ingot".into(),
            count: 1,
            components: String::new(),
        });
        for tick in 1..=50 {
            if tick % 8 == 0 {
                slots[0].as_mut().unwrap().count += 1;
            }
            let u = work_inventory(
                &state,
                &slots,
                "pressing",
                16.0,
                Heat::None,
                create_simulation::active_recipes(),
                |_| 64,
            )
            .unwrap();
            state = u.state;
            slots = u.inventory.after;
            if u.result == TickResult::Completed {
                assert_eq!(tick, 50);
                assert_eq!(slots[9].as_ref().unwrap().count, 7);
                assert!(slots[0].is_none());
                return;
            }
        }
        panic!("incoming stacks kept resetting depot work");
    }
    #[test]
    fn a_full_depot_cannot_consume_or_reroll_a_batch() {
        let mut state = MachineState::default();
        let mut slots = vec![None; 27];
        slots[0] = Some(Stack {
            id: "minecraft:iron_ingot".into(),
            count: 64,
            components: "{name:1}".into(),
        });
        for s in &mut slots[OUTPUT] {
            *s = Some(Stack {
                id: "minecraft:stone".into(),
                count: 64,
                components: String::new(),
            });
        }
        let original = slots.clone();
        let rng = state.rng;
        for _ in 0..50 {
            let u = work_inventory(
                &state,
                &slots,
                "pressing",
                16.0,
                Heat::None,
                create_simulation::active_recipes(),
                |_| 64,
            )
            .unwrap();
            state = u.state;
            assert_eq!(u.inventory.after, original);
        }
        assert_eq!(state.status, "OutputBlocked");
        assert_eq!(state.rng, rng);
        for s in &mut slots[OUTPUT] {
            *s = None;
        }
        let u = work_inventory(
            &state,
            &slots,
            "pressing",
            16.0,
            Heat::None,
            create_simulation::active_recipes(),
            |_| 64,
        )
        .unwrap();
        assert_eq!(u.result, TickResult::Completed);
        assert_eq!(u.inventory.after[9].as_ref().unwrap().count, 64);
    }
}

/// Bounded visual strokes; collision remains the static authored block shape.
pub fn press_stroke(progress: f32, clearance: f32) -> f32 {
    if !progress.is_finite() || !clearance.is_finite() {
        return 0.0;
    }
    (progress.clamp(0.0, 1.0) * std::f32::consts::PI)
        .sin()
        .max(0.0)
        * clearance.clamp(0.0, 1.0)
}
pub fn mixer_stroke(progress: f32, clearance: f32) -> f32 {
    if !progress.is_finite() || !clearance.is_finite() {
        return 0.0;
    }
    (progress / 0.15)
        .min((1.0 - progress) / 0.15)
        .clamp(0.0, 1.0)
        * clearance.clamp(0.0, 1.0)
}
#[cfg(test)]
mod animation_tests {
    use super::*;
    #[test]
    fn press_retracts_and_never_exceeds_its_gap() {
        assert_eq!(press_stroke(0.0, 0.9375), 0.0);
        assert_eq!(press_stroke(1.0, 0.9375), 0.0);
        assert_eq!(press_stroke(0.5, 0.9375), 0.9375);
        assert_eq!(press_stroke(f32::NAN, 1.0), 0.0);
    }
    #[test]
    fn mixer_lowers_holds_and_retracts() {
        assert_eq!(mixer_stroke(0.0, 0.8), 0.0);
        assert_eq!(mixer_stroke(0.5, 0.8), 0.8);
        assert_eq!(mixer_stroke(1.0, 0.8), 0.0);
        assert_eq!(mixer_stroke(-1.0, 0.8), 0.0);
    }
}

/// Two horizontal-axis wheels separated by one gap block. Positive-axis
/// ordering fixes which RPM signs turn the inner faces down into the gap.
pub fn crushing_pair(
    a: [i32; 3],
    b: [i32; 3],
    axis: super::kinetics::Axis,
    rpm_a: f32,
    rpm_b: f32,
) -> Option<([f64; 3], f32)> {
    if !rpm_a.is_finite() || !rpm_b.is_finite() || rpm_a == 0.0 || rpm_b == 0.0 {
        return None;
    }
    let d = [
        b[0].checked_sub(a[0])?,
        b[1].checked_sub(a[1])?,
        b[2].checked_sub(a[2])?,
    ];
    let inward = match axis {
        super::kinetics::Axis::X => d == [0, 0, 2] && rpm_a > 0.0 && rpm_b < 0.0,
        super::kinetics::Axis::Z => d == [2, 0, 0] && rpm_a < 0.0 && rpm_b > 0.0,
        _ => false,
    };
    inward.then_some((
        [
            (f64::from(a[0]) + f64::from(b[0])) * 0.5 + 0.5,
            f64::from(a[1]) + 0.25,
            (f64::from(a[2]) + f64::from(b[2])) * 0.5 + 0.5,
        ],
        rpm_a.abs().min(rpm_b.abs()),
    ))
}
pub fn in_crushing_gap(center: [f64; 3], item: [f64; 3]) -> bool {
    item.iter().all(|v| v.is_finite())
        && (item[0] - center[0]).abs() <= 0.45
        && (item[2] - center[2]).abs() <= 0.45
        && (-0.1..=1.25).contains(&(item[1] - center[1]))
}
#[cfg(test)]
mod crusher_tests {
    use super::super::kinetics::Axis;
    use super::*;
    #[test]
    fn only_inward_opposed_horizontal_wheels_form_a_pair() {
        let (center, rpm) = crushing_pair([0, 2, 0], [0, 2, 2], Axis::X, 32.0, -64.0).unwrap();
        assert_eq!(center, [0.5, 2.25, 1.5]);
        assert_eq!(rpm, 32.0);
        assert!(crushing_pair([0, 2, 0], [0, 2, 2], Axis::X, -32.0, 32.0).is_none());
        assert!(crushing_pair([0, 2, 0], [2, 2, 0], Axis::X, 32.0, -32.0).is_none());
        assert!(crushing_pair([0, 2, 0], [0, 4, 0], Axis::Y, 32.0, -32.0).is_none());
        assert!(crushing_pair([0, 2, 0], [2, 2, 0], Axis::Z, -32.0, 32.0).is_some());
        assert!(in_crushing_gap(center, [0.5, 3.0, 1.5]));
        assert!(!in_crushing_gap(center, [1.5, 3.0, 1.5]));
    }
    #[test]
    fn ore_stacks_crush_in_bulk_with_saved_random_state() {
        let input = Stack {
            id: "minecraft:raw_iron".into(),
            count: 16,
            components: String::new(),
        };
        let mut state = MachineState::default();
        for _ in 0..300 {
            let u = work(
                &state,
                41,
                &input,
                "crushing",
                32.0,
                Heat::None,
                create_simulation::active_recipes(),
                |_| 64,
            )
            .unwrap();
            state = MachineState::decode(&u.state.encode()).unwrap();
            if u.result == TickResult::Completed {
                assert!(u.remaining.is_none());
                assert!(
                    u.outputs
                        .iter()
                        .any(|s| s.id == "create:crushed_raw_iron" && s.count >= 16)
                );
                return;
            }
        }
        panic!("world ore stack did not crush");
    }
}
