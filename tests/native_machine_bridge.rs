use super::*;

fn named_iron() -> ItemStack {
    horizon_client::item_data::parse_stack(
        r#"{id:"minecraft:iron_ingot",count:3,components:{"minecraft:custom_name":{text:"Workshop iron"}}}"#,
    ).unwrap().unwrap()
}
#[test]
fn real_host_item_components_roundtrip_through_press() {
    let mut slots = vec![None; 27];
    slots[0] = Some(named_iron());
    let before = portable(&slots);
    assert!(!before[0].as_ref().unwrap().components.is_empty());
    let registry = machines::kinetics::state::Registry::new();
    let mut motor =
        machines::kinetics::state::parse_state(&registry, "create:creative_motor[facing=north]")
            .unwrap();
    motor.configured_rpm = Some(16.0);
    let press =
        machines::kinetics::state::parse_state(&registry, "create:mechanical_press[facing=north]")
            .unwrap();
    let nodes = std::collections::BTreeMap::from([([0, 0, 1], motor), ([0, 0, 0], press)]);
    let rpm = machines::kinetics::solve(&nodes).speeds[&[0, 0, 0]];
    assert_eq!(rpm, 16.0);
    let mut state = machines::MachineState::default();
    for _ in 0..100 {
        let update = machines::process(
            &state,
            &portable(&slots),
            "pressing",
            rpm,
            machines::Heat::None,
            |_| 64,
        )
        .unwrap();
        state = machines::MachineState::decode(&update.state.encode()).unwrap();
        slots = restore(&update.inventory.after, &slots).unwrap();
        if update.result == machines::TickResult::Completed {
            assert_eq!(slots[0].as_ref().unwrap().count, 2);
            assert_eq!(
                portable(&slots)[0].as_ref().unwrap().components,
                before[0].as_ref().unwrap().components
            );
            assert_eq!(&*slots[9].as_ref().unwrap().item, "create:iron_sheet");
            return;
        }
    }
    panic!("host inventory did not process");
}
#[test]
fn unpowered_host_items_and_components_stay_unchanged() {
    let mut slots = vec![None; 27];
    slots[0] = Some(named_iron());
    let input = portable(&slots);
    let update = machines::process(
        &machines::MachineState::default(),
        &input,
        "pressing",
        0.0,
        machines::Heat::None,
        |_| 64,
    )
    .unwrap();
    assert_eq!(update.result, machines::TickResult::NoPower);
    assert_eq!(restore(&update.inventory.after, &slots).unwrap(), slots);
}
#[test]
fn retained_host_tool_keeps_damage_and_components() {
    let mut slots = vec![None; 27];
    slots[0] = Some(ItemStack::new("minecraft:exposed_copper", 1));
    slots[1]=horizon_client::item_data::parse_stack(r#"{id:"minecraft:iron_axe",count:1,components:{"minecraft:damage":12,"minecraft:custom_name":{text:"Workshop tool"}}}"#).unwrap();
    let tool = slots[1].clone();
    let mut state = machines::MachineState::default();
    for _ in 0..100 {
        let update = machines::process(
            &state,
            &portable(&slots),
            "deploying",
            32.0,
            machines::Heat::None,
            |_| 64,
        )
        .unwrap();
        state = update.state;
        slots = restore(&update.inventory.after, &slots).unwrap();
        if update.result == machines::TickResult::Completed {
            assert!(slots[0].is_none());
            assert_eq!(slots[1], tool);
            assert_eq!(&*slots[9].as_ref().unwrap().item, "minecraft:copper_block");
            return;
        }
    }
    panic!("deployer did not retain its tool");
}
