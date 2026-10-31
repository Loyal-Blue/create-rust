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
fn component_inputs_process_and_surviving_items_keep_metadata() {
    let mut slots = press();
    slots[0].as_mut().unwrap().components = "{custom:1}".into();
    let u = finish(&MachineState::default(), &slots, |_| 64);
    assert_eq!(u.result, TickResult::Completed);
    let left = u.inventory.after[0].as_ref().unwrap();
    assert_eq!(left.count, 2);
    assert_eq!(left.components, "{custom:1}");
    assert_eq!(u.inventory.after[9], stack("create:iron_sheet", 1));
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

#[test]
fn geared_press_pipeline_survives_state_reload_and_conserves_items() {
    use kinetics::{Axis, Kind, Node, solve};
    let mut motor = Node::new(Kind::CreativeMotor, Axis::Z);
    motor.configured_rpm = Some(64.0);
    let nodes = BTreeMap::from([
        ([0, 0, 0], motor),
        ([0, 0, 1], Node::new(Kind::LargeCogwheel, Axis::Z)),
        ([1, 1, 1], Node::new(Kind::Cogwheel, Axis::Z)),
        ([1, 1, 2], Node::new(Kind::MechanicalPress, Axis::Z)),
    ]);
    let solution = solve(&nodes);
    let rpm = solution.speeds[&[1, 1, 2]];
    assert_eq!(rpm, -128.0);
    let mut state = MachineState::default();
    let mut slots = press();
    for _ in 0..200 {
        let u = process(&state, &slots, "pressing", rpm, Heat::None, |_| 64).unwrap();
        let result = u.result;
        state = MachineState::decode(&u.state.encode()).unwrap();
        assert!(u.inventory.commit(&mut slots));
        if matches!(result, TickResult::Completed) {
            break;
        }
    }
    assert_eq!(slots[0].as_ref().unwrap().count, 2);
    assert_eq!(slots[9], stack("create:iron_sheet", 1));
}
#[test]
fn stopped_drive_cannot_process_or_consume_input() {
    let before = press();
    let state = MachineState::default();
    let mut slots = before.clone();
    let mut state = state;
    for _ in 0..100 {
        let u = process(&state, &slots, "pressing", 0.0, Heat::None, |_| 64).unwrap();
        state = u.state.clone();
        assert!(u.inventory.commit(&mut slots));
    }
    assert_eq!(slots, before);
    assert_eq!(state.progress, 0.0);
}
#[test]
fn configured_motor_speed_is_validated_when_loading() {
    assert!(MachineState::decode("{\"motor_rpm\":257}").is_err());
    assert!(MachineState::decode("{\"motor_rpm\":-1}").is_err());
    assert_eq!(MachineState::decode("{}").unwrap().motor_rpm, 16.0);
}

#[test]
fn waterwheel_speed_press_finishes_within_five_seconds() {
    let mut state = MachineState::default();
    let slots = press();
    for tick in 1..=100 {
        let update = process(&state, &slots, "pressing", 8.0, Heat::None, |_| 64).unwrap();
        state = MachineState::decode(&update.state.encode()).unwrap();
        if update.result == TickResult::Completed {
            assert_eq!(tick, 100);
            assert_eq!(update.inventory.after[9], stack("create:iron_sheet", 1));
            return;
        }
    }
    panic!("waterwheel-powered press did not finish");
}
#[test]
fn local_inputs_and_progress_reasons_are_visible() {
    assert!(has_inputs(&press()));
    let mut slots = vec![None; 27];
    slots[9] = stack("create:iron_sheet", 1);
    assert!(!has_inputs(&slots));
    let state = MachineState {
        status: "TooSlow".into(),
        ..Default::default()
    };
    assert_eq!(status_label(&state), "Needs 32+ RPM");
}
#[test]
fn rotation_uses_elapsed_time_and_is_continuous_across_packets() {
    let mut state = MachineState::default();
    state.set_speed(64.0, 100);
    let first = RotationSample::capture(&state, 100);
    assert!((first.angle_after(0.125) - 48.0).abs() < 0.001);
    let second = RotationSample::capture(&state, 105);
    assert!((first.angle_after(0.25) - second.angle_after(0.0)).abs() < 0.001);
    assert_eq!(first.angle_after(100.0), first.angle_after(0.5));
    state.set_speed(0.0, 105);
    let stopped = RotationSample::capture(&state, 105);
    assert_eq!(stopped.angle_after(0.0), stopped.angle_after(0.25));
}

#[test]
fn press_over_basin_falls_back_without_resetting_progress() {
    let mut state = MachineState::default();
    let mut slots = press();
    for _ in 0..100 {
        let u = process(&state, &slots, "compacting", 16.0, Heat::None, |_| 64).unwrap();
        state = MachineState::decode(&u.state.encode()).unwrap();
        slots = u.inventory.after;
        if u.result == TickResult::Completed {
            assert_eq!(slots[9], stack("create:iron_sheet", 1));
            return;
        }
    }
    panic!("pressing fallback reset its progress");
}

#[test]
fn transport_settings_roundtrip_and_old_saves_keep_defaults() {
    let old = MachineState::decode(r#"{"version":1,"rpm":16.0}"#).unwrap();
    assert!(!old.extracting);
    assert_eq!(old.transfer_limit, 1);
    let state = MachineState {
        extracting: true,
        item_filter: Some("minecraft:iron_ingot".into()),
        transfer_limit: 64,
        ..Default::default()
    };
    assert_eq!(MachineState::decode(&state.encode()).unwrap(), state);
    let mut bad = state.clone();
    bad.transfer_limit = 0;
    assert!(MachineState::decode(&bad.encode()).is_err());
    bad = state;
    bad.item_filter = Some("invalid".into());
    assert!(MachineState::decode(&bad.encode()).is_err());
}

#[test]
fn pump_meshes_with_a_cog_while_both_pipe_ports_stay_connected() {
    use kinetics::state::{Registry, parse_state};
    let r = Registry::new();
    let pump = parse_state(&r, "create:mechanical_pump[facing=east]").unwrap();
    let cog = parse_state(&r, "create:cogwheel[axis=x]").unwrap();
    let mut motor = parse_state(&r, "create:creative_motor[facing=west]").unwrap();
    motor.configured_rpm = Some(32.0);
    motor.direction = -1;
    let solved = kinetics::solve(&BTreeMap::from([
        ([0, 0, 0], pump),
        ([0, 1, 0], cog),
        ([1, 1, 0], motor),
    ]));
    let rpm = solved.speeds[&[0, 0, 0]];
    assert_eq!(rpm, 32.0);
    let nodes = BTreeMap::from([
        ([-1, 0, 0], fluids::Node::SourceWater),
        ([1, 0, 0], fluids::Node::Pipe),
        ([2, 0, 0], fluids::Node::Port { single: true }),
    ]);
    let flow = fluids::flow(
        &nodes,
        &BTreeMap::from([([2, 0, 0], MachineState::default())]),
        [0, 0, 0],
        kinetics::Axis::X,
        rpm,
        0,
    );
    assert_eq!(flow.amount, 128);
    assert_eq!(flow.states[&[2, 0, 0]].fluid_in["minecraft:water"], 128);
}
