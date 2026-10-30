use super::*;
fn recipe(id: &str) -> &'static Recipe {
    active_recipes().iter().find(|r| r.id == id).unwrap()
}
fn finish(m: &mut Machine, rpm: f32) -> TickResult {
    for _ in 0..10000 {
        let r = m.tick(rpm);
        if !matches!(r, TickResult::Working) {
            return r;
        }
    }
    panic!("recipe never finished")
}
#[test]
fn actual_iron_pressing_consumes_exactly_one_ingot() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 2, false).unwrap();
    assert_eq!(finish(&mut m, 32.0), TickResult::Completed);
    assert_eq!(m.input.items["minecraft:iron_ingot"], 1);
    assert_eq!(m.output.items["create:iron_sheet"], 1);
}
#[test]
fn no_power_preserves_inputs_and_work() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick(32.0), TickResult::Working);
    let before = m.encode();
    for rpm in [0.0, f32::NAN, f32::INFINITY] {
        assert_eq!(m.tick(rpm), TickResult::NoPower);
        assert_eq!(m.encode(), before);
    }
}
#[test]
fn blocked_output_cannot_consume_inputs_or_reroll() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    m.output.slots = 0;
    assert_eq!(finish(&mut m, 256.0), TickResult::OutputBlocked);
    let saved = m.encode();
    for _ in 0..20 {
        assert_eq!(m.tick(256.0), TickResult::OutputBlocked);
        assert_eq!(m.encode(), saved);
    }
    m.output.slots = 9;
    assert_eq!(m.tick(256.0), TickResult::Completed);
    assert!(m.input.items.is_empty());
}
#[test]
fn stock_insertion_is_atomic_and_checks_stack_and_fluid_limits() {
    let mut s = Stock {
        slots: 1,
        ..Stock::default()
    };
    s.insert("minecraft:iron_ingot", 64, false).unwrap();
    let old = s.clone();
    assert!(s.insert("minecraft:gold_ingot", 1, false).is_err());
    assert_eq!(s, old);
    assert!(s.insert("minecraft:water", 4001, true).is_err());
    assert_eq!(s, old);
    assert!(s.insert("bad id", 1, false).is_err());
    assert!(s.insert("minecraft:water", 0, true).is_err());
}
#[test]
fn hot_mixing_requires_heat_and_at_least_32_rpm() {
    let r = recipe("create:mixing/brass_ingot");
    let mut m = Machine::new("mixing");
    for i in r.ingredients {
        let id = i
            .matcher
            .strip_prefix('#')
            .and_then(|tag| catalog::tags().iter().find(|(t, _)| *t == tag))
            .map(|(_, ids)| ids[0])
            .unwrap_or(i.matcher);
        m.input.insert(id, i.count, i.fluid).unwrap();
    }
    assert_eq!(m.tick(16.0), TickResult::TooSlow);
    assert_eq!(m.tick(32.0), TickResult::Cold);
    m.heat = Heat::Superheated;
    assert_eq!(finish(&mut m, 64.0), TickResult::Completed);
}
#[test]
fn fluid_filling_reserves_both_the_bottle_and_fluid() {
    let mut m = Machine::new("filling");
    m.input.insert("minecraft:glass_bottle", 1, false).unwrap();
    m.input.insert("create:honey", 250, true).unwrap();
    assert_eq!(finish(&mut m, 32.0), TickResult::Completed);
    assert!(m.input.items.is_empty());
    assert!(m.input.fluids.is_empty());
    assert_eq!(m.output.items["minecraft:honey_bottle"], 1);
}
#[test]
fn save_reload_preserves_recipe_progress_and_random_state() {
    let mut m = Machine::new("milling");
    m.input.insert("minecraft:wheat", 5, false).unwrap();
    for _ in 0..7 {
        m.tick(32.0);
    }
    let mut restored = Machine::decode(&m.encode()).unwrap();
    assert_eq!(m.encode(), restored.encode());
    for _ in 0..300 {
        assert_eq!(m.tick(32.0), restored.tick(32.0));
    }
    assert_eq!(m.encode(), restored.encode());
}
#[test]
fn malformed_saves_fail_closed() {
    for text in [
        "garbage",
        "CHLAB1 pressing NaN 0 1\n",
        "CHLAB1 pressing -1 0 1\n",
        "CHLAB1 pressing 0 4 1\n",
        "CHLAB1 pressing 0 0 1\nin item minecraft:iron_ingot 999999\n",
        "CHLAB1 pressing 0 0 1\nrecipe missing\n",
    ] {
        assert!(Machine::decode(text).is_err(), "{text}");
    }
}
#[test]
fn repeated_ingredients_cannot_share_a_single_item() {
    static R: Recipe = Recipe {
        id: "test:repeat",
        station: "pressing",
        ingredients: &[
            Ingredient {
                matcher: "minecraft:iron_ingot",
                count: 1,
                fluid: false,
                consume: true,
            },
            Ingredient {
                matcher: "minecraft:iron_ingot",
                count: 1,
                fluid: false,
                consume: true,
            },
        ],
        products: &[Product {
            id: "create:iron_sheet",
            count: 1,
            fluid: false,
            chance: 1.0,
        }],
        work: 1,
        heat: Heat::None,
    };
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Idle);
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Completed);
    assert!(m.input.items.is_empty());
}
#[test]
fn retained_tools_are_reserved_and_returned_after_a_craft() {
    static R: Recipe = Recipe {
        id: "test:tool",
        station: "deploying",
        ingredients: &[
            Ingredient {
                matcher: "minecraft:oak_log",
                count: 1,
                fluid: false,
                consume: true,
            },
            Ingredient {
                matcher: "minecraft:iron_axe",
                count: 1,
                fluid: false,
                consume: false,
            },
        ],
        products: &[Product {
            id: "minecraft:stripped_oak_log",
            count: 1,
            fluid: false,
            chance: 1.0,
        }],
        work: 1,
        heat: Heat::None,
    };
    let mut m = Machine::new("deploying");
    m.input.insert("minecraft:oak_log", 1, false).unwrap();
    m.input.insert("minecraft:iron_axe", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Completed);
    assert_eq!(m.input.items["minecraft:iron_axe"], 1);
    assert!(!m.input.items.contains_key("minecraft:oak_log"));
}
#[test]
fn every_imported_recipe_can_complete_with_valid_inputs() {
    for r in active_recipes() {
        let mut m = Machine::new(r.station);
        m.heat = Heat::Superheated;
        for i in r.ingredients {
            let id = if let Some(tag) = i.matcher.strip_prefix('#') {
                catalog::tags().iter().find(|(t, _)| *t == tag).unwrap().1[0]
            } else {
                i.matcher
            };
            m.input
                .insert(id, i.count, i.fluid)
                .unwrap_or_else(|e| panic!("{}: {e}", r.id));
        }
        // Select this recipe explicitly: the same ingredients may have several recipes.
        m.selected = Some(r.id.into());
        assert_eq!(finish(&mut m, 256.0), TickResult::Completed, "{}", r.id);
    }
}
