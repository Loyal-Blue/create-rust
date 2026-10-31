//! Deployer applications over a live target and the deployer's real held inputs.
//! The host commits both snapshots together; no item is stored in machine state.
use super::{Heat, INPUT, InventoryPlan, MachineState, OUTPUT, Stack, TickResult, surface};
use create_simulation::{Recipe, catalog};

fn matches(matcher: &str, id: &str) -> bool {
    matcher.strip_prefix('#').map_or(matcher == id, |tag| {
        catalog::tags()
            .iter()
            .any(|(name, ids)| *name == tag && ids.contains(&id))
    })
}
/// Original ingredient order matters: target first, held material/tool second.
/// Component-dependent, fluid and heated applications need separate host APIs.
pub fn select<'a>(
    input: &Stack,
    hand: &[Option<Stack>],
    recipes: &'a [Recipe],
) -> Option<(&'a Recipe, usize)> {
    recipes.iter().find_map(|r| {
        if !matches!(r.station, "deploying" | "item_application")
            || r.heat != Heat::None
            || r.ingredients.len() != 2
            || r.ingredients.iter().any(|i| i.fluid || i.count == 0)
            || r.products.iter().any(|p| p.fluid)
        {
            return None;
        }
        let base = &r.ingredients[0];
        let tool = &r.ingredients[1];
        if !base.consume || input.count < base.count || !matches(base.matcher, &input.id) {
            return None;
        }
        hand.iter().take(INPUT.end).enumerate().find_map(|(i, s)| {
            s.as_ref()
                .filter(|s| s.count >= tool.count && matches(tool.matcher, &s.id))
                .map(|_| (r, i))
        })
    })
}
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub entity: u32,
    pub position: [i32; 3],
}
#[derive(Clone, Debug)]
pub struct Application {
    original: MachineState,
    pub state: MachineState,
    pub before: Stack,
    pub remaining: Option<Stack>,
    pub outputs: Vec<Stack>,
    pub hand: InventoryPlan,
    pub result: TickResult,
}
/// A processed output must leave this applicator before another operation.
/// Material identity survives host stack merges that change only its count.
pub fn applied(state: &MachineState, entity: u32, stack: &Stack) -> bool {
    state
        .applied_entities
        .iter()
        .any(|b| b.entity == entity && b.material == surface::material(stack))
}
pub fn remember(state: &mut MachineState, entity: u32, stack: &Stack) -> bool {
    let binding = surface::Binding {
        entity,
        fingerprint: surface::fingerprint(stack),
        material: surface::material(stack),
        count: stack.count,
    };
    if let Some(old) = state
        .applied_entities
        .iter_mut()
        .find(|b| b.entity == entity)
    {
        *old = binding;
        return true;
    }
    if state.applied_entities.len() >= 128 {
        return false;
    }
    state.applied_entities.push(binding);
    true
}
/// Exactly one recipe operation, including bulk target leftovers and retained
/// component-bearing tools. A changed target or held item restarts its work.
pub fn work(
    state: &MachineState,
    target: Target,
    input: &Stack,
    hand: &[Option<Stack>],
    rpm: f32,
    recipes: &[Recipe],
    limit: impl Fn(&str) -> u32,
) -> Result<Option<Application>, String> {
    if hand.len() < OUTPUT.end {
        return Err("Deployer needs real input/output slots".into());
    }
    if applied(state, target.entity, input) {
        return Ok(None);
    }
    let Some((recipe, slot)) = select(input, hand, recipes) else {
        return Ok(None);
    };
    let tool = hand[slot].as_ref().unwrap();
    let place = target
        .position
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
    let binding = surface::Binding {
        entity: target.entity,
        fingerprint: surface::fingerprint(input),
        material: surface::material(input).wrapping_mul(0x100000001b3)
            ^ surface::fingerprint(tool).rotate_left(17)
            ^ slot as u64
            ^ place,
        count: input.count,
    };
    let mut bound = state.clone();
    if bound.world_target != Some(binding) {
        bound.progress = 0.;
        bound.recipe = None;
    }
    bound.world_target = Some(binding);
    let mut scratch = vec![None; 27];
    scratch[0] = Some(input.clone());
    scratch[1] = Some(tool.clone());
    let update = super::process_with_catalog(
        &bound,
        &scratch,
        recipe.station,
        rpm,
        Heat::None,
        std::slice::from_ref(recipe),
        limit,
    )?;
    let mut next = update.state;
    let mut after = hand.to_vec();
    let mut remaining = Some(input.clone());
    let mut outputs = vec![];
    if update.result == TickResult::Completed {
        let count = input.count - recipe.ingredients[0].count;
        remaining = (count > 0).then(|| Stack {
            count,
            ..input.clone()
        });
        if recipe.ingredients[1].consume {
            let count = tool.count - recipe.ingredients[1].count;
            after[slot] = (count > 0).then(|| Stack {
                count,
                ..tool.clone()
            });
        }
        outputs = update.inventory.after[OUTPUT]
            .iter()
            .flatten()
            .cloned()
            .collect();
        next.world_target = None;
    }
    Ok(Some(Application {
        original: state.clone(),
        state: next,
        before: input.clone(),
        remaining,
        outputs,
        hand: InventoryPlan {
            before: hand.to_vec(),
            after,
        },
        result: update.result,
    }))
}

/// Reserve a depot output row before consuming either the target or held item.
impl Application {
    pub fn depot(
        mut self,
        slots: &[Option<Stack>],
        slot: usize,
        limit: impl Fn(&str) -> u32,
    ) -> Result<(Self, InventoryPlan), String> {
        if slots.len() < OUTPUT.end
            || slot >= INPUT.end
            || slots[slot].as_ref() != Some(&self.before)
        {
            return Err("Stale depot target".into());
        }
        let mut after = slots.to_vec();
        if self.result == TickResult::Completed {
            after[slot] = self.remaining.clone();
            if !self
                .outputs
                .iter()
                .all(|s| super::insert(&mut after[OUTPUT], s, &limit))
            {
                after = slots.to_vec();
                self.hand.after = self.hand.before.clone();
                self.remaining = Some(self.before.clone());
                self.outputs.clear();
                self.state = self.original.clone();
                self.state.status = "OutputBlocked".into();
                self.result = TickResult::OutputBlocked;
            }
        }
        Ok((
            self,
            InventoryPlan {
                before: slots.to_vec(),
                after,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use create_simulation::{Ingredient, Product};
    fn t(entity: u32) -> Target {
        Target {
            entity,
            position: [0; 3],
        }
    }
    fn s(id: &str, count: u32) -> Stack {
        Stack {
            id: id.into(),
            count,
            components: String::new(),
        }
    }
    fn hand(tool: Stack) -> Vec<Option<Stack>> {
        let mut v = vec![None; 27];
        v[0] = Some(tool);
        v
    }
    fn cog() -> Recipe {
        *create_simulation::active_recipes()
            .iter()
            .find(|r| r.id == "create:deploying/cogwheel")
            .unwrap()
    }
    fn done(input: &Stack, hand: &[Option<Stack>], recipe: Recipe) -> Application {
        let mut state = MachineState::default();
        for _ in 0..1000 {
            let u = work(&state, t(42), input, hand, 32., &[recipe], |_| 64)
                .unwrap()
                .unwrap();
            state = MachineState::decode(&u.state.encode()).unwrap();
            if u.result == TickResult::Completed {
                return u;
            }
        }
        panic!("never completed")
    }
    #[test]
    fn bulk_targets_consume_one_material_and_preserve_both_remainders() {
        let mut input = s("create:shaft", 4);
        input.components = "{custom:1}".into();
        let mut tool = s("minecraft:oak_planks", 8);
        tool.components = "{custom:2}".into();
        let h = hand(tool.clone());
        let u = done(&input, &h, cog());
        assert_eq!(u.outputs, vec![s("create:cogwheel", 1)]);
        assert_eq!(u.remaining, Some(Stack { count: 3, ..input }));
        assert_eq!(u.hand.after[0], Some(Stack { count: 7, ..tool }));
        assert_eq!(u.hand.before, h);
    }
    #[test]
    fn retained_named_damaged_tool_is_unchanged() {
        let r = *create_simulation::active_recipes()
            .iter()
            .find(|r| r.id == "create:deploying/copper_block_from_deoxidising")
            .unwrap();
        let mut axe = s("minecraft:iron_axe", 1);
        axe.components = "{\"minecraft:damage\":7,\"minecraft:custom_name\":\"Axe\"}".into();
        let h = hand(axe);
        let u = done(&s("minecraft:exposed_copper", 1), &h, r);
        assert_eq!(u.hand.after, h);
        assert!(u.remaining.is_none());
        assert_eq!(u.outputs, vec![s("minecraft:copper_block", 1)]);
    }
    #[test]
    fn zero_power_does_not_consume_or_advance() {
        let i = s("create:shaft", 1);
        let h = hand(s("minecraft:oak_planks", 1));
        let u = work(&MachineState::default(), t(4), &i, &h, 0., &[cog()], |_| 64)
            .unwrap()
            .unwrap();
        assert_eq!(u.result, TickResult::NoPower);
        assert_eq!(u.hand.after, h);
        assert_eq!(u.remaining, Some(i));
        assert!(u.outputs.is_empty());
        assert_eq!(u.state.progress, 0.);
    }
    #[test]
    fn input_or_tool_changes_reset_saved_progress() {
        let i = s("create:shaft", 2);
        let h = hand(s("minecraft:oak_planks", 4));
        let mut state = MachineState::default();
        for _ in 0..5 {
            state = work(&state, t(4), &i, &h, 8., &[cog()], |_| 64)
                .unwrap()
                .unwrap()
                .state;
        }
        let first = work(&MachineState::default(), t(4), &i, &h, 8., &[cog()], |_| 64)
            .unwrap()
            .unwrap();
        assert!(state.progress > first.state.progress);
        let mut changed = h.clone();
        changed[0].as_mut().unwrap().components = "{changed:1}".into();
        let u = work(&state, t(4), &i, &changed, 8., &[cog()], |_| 64)
            .unwrap()
            .unwrap();
        assert_eq!(u.state.progress, first.state.progress);
        let u = work(&state, t(5), &i, &h, 8., &[cog()], |_| 64)
            .unwrap()
            .unwrap();
        assert_eq!(u.state.progress, first.state.progress);
        let u = work(
            &state,
            Target {
                entity: 4,
                position: [1, 0, 0],
            },
            &i,
            &h,
            8.,
            &[cog()],
            |_| 64,
        )
        .unwrap()
        .unwrap();
        assert_eq!(u.state.progress, first.state.progress);
        let u = work(
            &state,
            t(4),
            &s("create:shaft", 3),
            &h,
            8.,
            &[cog()],
            |_| 64,
        )
        .unwrap()
        .unwrap();
        assert_eq!(u.state.progress, first.state.progress);
    }
    #[test]
    fn stale_hand_plan_cannot_commit() {
        let i = s("create:shaft", 1);
        let h = hand(s("minecraft:oak_planks", 2));
        let u = done(&i, &h, cog());
        let mut live = h;
        live[0].as_mut().unwrap().count = 3;
        let before = live.clone();
        assert!(!u.hand.commit(&mut live));
        assert_eq!(live, before);
    }
    #[test]
    fn target_and_held_ingredients_are_not_interchangeable() {
        assert!(
            select(
                &s("minecraft:oak_planks", 1),
                &hand(s("create:shaft", 1)),
                &[cog()]
            )
            .is_none()
        );
        assert!(
            select(
                &s("create:shaft", 1),
                &hand(s("minecraft:stone", 1)),
                &[cog()]
            )
            .is_none()
        );
    }
    #[test]
    fn repeated_identical_ingredients_stay_partitioned() {
        let r = Recipe {
            id: "create:test/same",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:stone",
                    count: 2,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:stone",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:andesite",
                count: 1,
                fluid: false,
                chance: 1.,
            }],
            work: 1,
            heat: Heat::None,
        };
        let u = done(&s("minecraft:stone", 3), &hand(s("minecraft:stone", 4)), r);
        assert_eq!(u.remaining.unwrap().count, 1);
        assert_eq!(u.hand.after[0].as_ref().unwrap().count, 3);
    }

    #[test]
    fn full_depot_preserves_input_held_material_and_rng() {
        let i = s("create:shaft", 1);
        let h = hand(s("minecraft:oak_planks", 2));
        let mut depot = vec![None; 27];
        depot[0] = Some(i.clone());
        for slot in &mut depot[9..18] {
            *slot = Some(s("minecraft:dirt", 64));
        }
        let u = done(&i, &h, cog());
        let rng = u.original.rng;
        let (u, plan) = u.depot(&depot, 0, |_| 64).unwrap();
        assert_eq!(u.result, TickResult::OutputBlocked);
        assert_eq!(u.state.rng, rng);
        assert_eq!(u.hand.after, h);
        assert_eq!(plan.after, depot);
    }
    #[test]
    fn depot_reserves_product_in_output_row() {
        let i = s("create:shaft", 2);
        let h = hand(s("minecraft:oak_planks", 2));
        let mut depot = vec![None; 27];
        depot[0] = Some(i.clone());
        let (u, plan) = done(&i, &h, cog()).depot(&depot, 0, |_| 64).unwrap();
        assert_eq!(u.result, TickResult::Completed);
        assert_eq!(plan.after[0].as_ref().unwrap().count, 1);
        assert_eq!(plan.after[9], Some(s("create:cogwheel", 1)));
        let mut changed = depot;
        changed[0] = None;
        assert!(!plan.commit(&mut changed));
    }
    #[test]
    fn blocked_depot_resumes_without_duplicate_material_consumption() {
        let input = s("create:shaft", 2);
        let held = hand(s("minecraft:oak_planks", 2));
        let mut depot = vec![None; 27];
        depot[0] = Some(input.clone());
        for slot in &mut depot[9..18] {
            *slot = Some(s("minecraft:dirt", 64));
        }
        let (blocked, _) = done(&input, &held, cog()).depot(&depot, 0, |_| 64).unwrap();
        let mut state = blocked.state;
        depot[9] = None;
        for _ in 0..1000 {
            let u = work(&state, t(42), &input, &held, 32., &[cog()], |_| 64)
                .unwrap()
                .unwrap();
            let (u, plan) = u.depot(&depot, 0, |_| 64).unwrap();
            state = u.state;
            if u.result == TickResult::Completed {
                assert_eq!(u.hand.after[0].as_ref().unwrap().count, 1);
                assert_eq!(plan.after[0].as_ref().unwrap().count, 1);
                assert_eq!(plan.after[9], Some(s("create:cogwheel", 1)));
                return;
            }
        }
        panic!("blocked operation never resumed");
    }
    #[test]
    fn processed_outputs_are_not_applied_again_after_stack_merging() {
        let mut state = MachineState::default();
        let product = s("create:cogwheel", 1);
        assert!(remember(&mut state, 7, &product));
        let state = MachineState::decode(&state.encode()).unwrap();
        let merged = s("create:cogwheel", 32);
        assert!(applied(&state, 7, &merged));
        assert!(!applied(&state, 8, &merged));
        assert!(!applied(&state, 7, &s("create:shaft", 1)));
        assert!(
            work(
                &state,
                t(7),
                &merged,
                &hand(s("minecraft:oak_planks", 64)),
                32.,
                create_simulation::active_recipes(),
                |_| 64
            )
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn processed_entity_state_is_bounded_and_older_saves_default_empty() {
        let mut state = MachineState::decode("{}").unwrap();
        assert!(state.applied_entities.is_empty());
        for id in 0..128 {
            assert!(remember(&mut state, id, &s("create:cogwheel", 1)));
        }
        assert!(!remember(&mut state, 129, &s("create:cogwheel", 1)));
        assert!(MachineState::decode(&state.encode()).is_ok());
        state.applied_entities.push(state.applied_entities[0]);
        assert!(MachineState::decode(&state.encode()).is_err());
    }
}
