use super::*;
fn stack(id: &str, count: u32) -> Option<Stack> {
    Some(Stack {
        id: id.into(),
        count,
        components: String::new(),
    })
}
fn press() -> Vec<Option<Stack>> {
    let mut slots = vec![None; 27];
    slots[0] = stack("minecraft:iron_ingot", 3);
    slots
}
fn finish(state: &MachineState, slots: &[Option<Stack>], limit: impl Fn(&str) -> u32) -> Update {
    let mut next = state.clone();
    for _ in 0..200 {
        let u = process(&next, slots, "pressing", 256., Heat::None, &limit).unwrap();
        next = u.state.clone();
        if matches!(u.result, TickResult::Completed | TickResult::OutputBlocked) {
            return u;
        }
    }
    panic!("recipe never completed")
}
#[test]
fn processes_real_slots_only() {
    let u = finish(&MachineState::default(), &press(), |_| 64);
    assert_eq!(u.result, TickResult::Completed);
    assert_eq!(u.inventory.after[0].as_ref().unwrap().count, 2);
    assert_eq!(u.inventory.after[9], stack("create:iron_sheet", 1));
    assert!(
        serde_json::from_str::<serde_json::Value>(&u.state.encode())
            .unwrap()
            .get("items")
            .is_none()
    );
}
#[test]
fn stale_inventory_cannot_commit() {
    let mut slots = press();
    let u = finish(&MachineState::default(), &slots, |_| 64);
    slots[0] = stack("minecraft:gold_ingot", 1);
    let before = slots.clone();
    assert!(!u.inventory.commit(&mut slots));
    assert_eq!(slots, before);
}
#[test]
fn component_inputs_are_preserved() {
    let mut slots = press();
    slots[0].as_mut().unwrap().components = "{custom:1}".into();
    let u = process(
        &MachineState::default(),
        &slots,
        "pressing",
        256.,
        Heat::None,
        |_| 64,
    )
    .unwrap();
    assert_eq!(u.result, TickResult::Idle);
    assert_eq!(u.inventory.after, slots);
}
#[test]
fn full_output_preserves_inputs_and_rng() {
    let mut slots = press();
    for s in &mut slots[OUTPUT] {
        *s = stack("create:iron_sheet", 1);
    }
    let state = MachineState::default();
    let u = finish(&state, &slots, |_| 1);
    assert_eq!(u.result, TickResult::OutputBlocked);
    assert_eq!(u.inventory.after, slots);
    assert_eq!(u.state.rng, state.rng);
}
#[test]
fn fluid_capacity_and_roundtrip() {
    let mut s = MachineState::default();
    assert!(s.insert_fluid("minecraft:water", 8000));
    assert!(!s.insert_fluid("minecraft:lava", 1));
    assert_eq!(MachineState::decode(&s.encode()).unwrap(), s);
    assert!(!s.extract_fluid("minecraft:water", 8001));
    assert!(s.extract_fluid("minecraft:water", 1000));
    assert_eq!(s.fluid_in["minecraft:water"], 7000);
}
#[test]
fn invalid_saved_data_is_rejected() {
    assert!(MachineState::decode("{\"version\":2}").is_err());
    assert!(MachineState::decode("{\"direction\":0}").is_err());
    assert!(MachineState::decode("{\"fluid_in\":{\"minecraft:water\":8001}}").is_err());
}
#[test]
fn move_preserves_components_and_actual_limits() {
    let mut from = vec![stack("minecraft:diamond_sword", 1)];
    from[0].as_mut().unwrap().components = "{damage:7}".into();
    let target = vec![None; 2];
    let (a, b) = transfer(&from, &target, 1, |_| 1).unwrap();
    assert_eq!(a.after, vec![None]);
    assert_eq!(b.after[0], from[0]);
    assert_eq!(from[0].as_ref().unwrap().count, 1);
}
#[test]
fn blocked_transfer_never_consumes() {
    let from = vec![stack("minecraft:stone", 2)];
    let to = vec![stack("minecraft:stone", 64)];
    assert!(transfer(&from, &to, 1, |_| 64).is_none());
}
#[test]
fn fluid_transfer_conserves_total_and_budget() {
    let mut from = MachineState::default();
    assert!(from.insert_fluid("minecraft:water", 3000));
    let to = MachineState::default();
    let (from, to, n) = transfer_fluid(&from, &to, 120).unwrap();
    assert_eq!(n, 120);
    assert_eq!(
        from.fluid_in["minecraft:water"] + to.fluid_in["minecraft:water"],
        3000
    );
}
#[test]
fn rotor_phase_survives_speed_changes_and_reload() {
    let mut s = MachineState::default();
    s.set_speed(32., 10);
    let angle = s.render_angle(27, 0.);
    s.set_speed(-64., 27);
    assert!((s.render_angle(27, 0.) - angle).abs() < 0.0001);
    assert_eq!(
        MachineState::decode(&s.encode())
            .unwrap()
            .render_angle(45, 0.5),
        s.render_angle(45, 0.5)
    );
}
