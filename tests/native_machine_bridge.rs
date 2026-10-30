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

#[test]
fn funnel_world_stack_and_machine_pipeline_preserves_items() {
    let mut chest = vec![None; 27];
    chest[0] = Some(named_iron());
    let original = chest.clone();
    let (out, world) = machines::transfer(&portable(&chest), &[None], 1, |_| 64).unwrap();
    chest = restore(&out.after, &chest).unwrap();
    let world = restore(&world.after, &original).unwrap();
    let input = vec![None; 27];
    let (world_after, insert) =
        machines::transfer(&portable(&world), &portable(&input), 1, |_| 64).unwrap();
    assert!(world_after.after[0].is_none());
    let mut mill = restore(&insert.after, &world).unwrap();
    let mut state = machines::MachineState::default();
    for _ in 0..100 {
        let u = machines::process(
            &state,
            &portable(&mill),
            "pressing",
            16.0,
            machines::Heat::None,
            |_| 64,
        )
        .unwrap();
        state = u.state;
        mill = restore(&u.inventory.after, &mill).unwrap();
        if u.result == machines::TickResult::Completed {
            break;
        }
    }
    let range = machines::transport::source_slots("create:mechanical_press", mill.len());
    let (source, output) =
        machines::transfer(&portable(&mill[range]), &[None], 64, |_| 64).unwrap();
    assert!(source.after.iter().all(Option::is_none));
    assert_eq!(output.after[0].as_ref().unwrap().id, "create:iron_sheet");
    assert_eq!(output.after[0].as_ref().unwrap().count, 1);
    assert_eq!(chest[0].as_ref().unwrap().count, 2);
    assert_eq!(
        portable(&chest)[0].as_ref().unwrap().components,
        portable(&original)[0].as_ref().unwrap().components
    );
}

#[test]
fn bulk_surface_processing_uses_real_named_host_stacks() {
    let mut iron = named_iron();
    iron.count = 64;
    let input = portable(&[Some(iron.clone())])[0].clone().unwrap();
    let mut state = machines::MachineState::default();
    for _ in 0..50 {
        let u = machines::surface::work(
            &state,
            99,
            &input,
            "pressing",
            16.0,
            machines::Heat::None,
            recipes::active_recipes(),
            |_| 64,
        )
        .unwrap();
        state = machines::MachineState::decode(&u.state.encode()).unwrap();
        if u.result == machines::TickResult::Completed {
            assert!(u.remaining.is_none());
            let output = restore(&[Some(u.outputs[0].clone())], &[Some(iron)]).unwrap()[0]
                .clone()
                .unwrap();
            assert_eq!(&*output.item, "create:iron_sheet");
            assert_eq!(output.count, 64);
            return;
        }
    }
    panic!("world stack did not process");
}
#[test]
fn depot_display_codec_preserves_components_without_registering_items() {
    let iron = named_iron();
    let data = portable(&[Some(iron.clone())])[0].clone().unwrap();
    let encoded = serde_json::to_string(&data).unwrap();
    let decoded: machines::Stack = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decode_portable_item(&decoded), Some(iron));
    let mut invalid = data.clone();
    invalid.count = 0;
    assert!(decode_portable_item(&invalid).is_none());
    invalid = data;
    invalid.components = "{".into();
    assert!(decode_portable_item(&invalid).is_none());
}

#[test]
fn water_filling_creates_a_real_water_bottle_with_correct_contents() {
    let input = ItemStack::new("minecraft:glass_bottle", 1);
    let stack = portable(&[Some(input.clone())])[0].clone().unwrap();
    let mut state = machines::MachineState::default();
    assert!(state.insert_fluid("minecraft:water", 250));
    for _ in 0..30 {
        let u = machines::surface::work(
            &state,
            8,
            &stack,
            "filling",
            32.0,
            machines::Heat::None,
            recipes::active_recipes(),
            |id| if id == "minecraft:potion" { 1 } else { 64 },
        )
        .unwrap();
        state = u.state;
        if u.result == machines::TickResult::Completed {
            let output = restore(&[Some(u.outputs[0].clone())], &[Some(input)]).unwrap()[0]
                .clone()
                .unwrap();
            assert_eq!(&*output.item, "minecraft:potion");
            assert_eq!(
                output.data().and_then(|d| d.potion.as_deref()),
                Some("minecraft:water")
            );
            assert!(state.fluid_in.is_empty());
            return;
        }
    }
    panic!("water bottle did not fill");
}
#[test]
fn generic_bucket_filling_and_emptying_conserve_a_bucket_of_water() {
    let mut state = machines::MachineState::default();
    assert!(state.insert_fluid("minecraft:water", 1000));
    let mut slots = vec![None; 27];
    slots[0] = Some(ItemStack::new("minecraft:bucket", 1));
    for _ in 0..30 {
        let u = machines::process(
            &state,
            &portable(&slots),
            "filling",
            32.0,
            machines::Heat::None,
            |_| 64,
        )
        .unwrap();
        state = u.state;
        slots = restore(&u.inventory.after, &slots).unwrap();
        if u.result == machines::TickResult::Completed {
            break;
        }
    }
    assert!(state.fluid_in.is_empty());
    assert_eq!(&*slots[9].as_ref().unwrap().item, "minecraft:water_bucket");
    slots[0] = slots[9].take();
    for _ in 0..30 {
        let u = machines::process(
            &state,
            &portable(&slots),
            "emptying",
            32.0,
            machines::Heat::None,
            |_| 64,
        )
        .unwrap();
        state = u.state;
        slots = restore(&u.inventory.after, &slots).unwrap();
        if u.result == machines::TickResult::Completed {
            break;
        }
    }
    assert_eq!(state.fluid_out["minecraft:water"], 1000);
    assert_eq!(&*slots[9].as_ref().unwrap().item, "minecraft:bucket");
}

#[test]
fn world_saw_outputs_and_named_remainder_restore_to_host_stacks() {
    let input=horizon_client::item_data::parse_stack(r#"{id:"minecraft:stripped_oak_log",count:2,components:{"minecraft:custom_name":{text:"Sawmill batch"}}}"#).unwrap().unwrap();
    let original=portable(&[Some(input.clone())]);
    let catalog:Vec<_>=recipes::active_recipes().iter().filter(|r|r.station=="cutting" && recipes::output_matches(r,Some("minecraft:oak_planks"))).copied().collect();
    let mut state=machines::MachineState::default();
    for _ in 0..100 {
        let u=machines::surface::work(&state,91,original[0].as_ref().unwrap(),"cutting",32.0,machines::Heat::None,&catalog,|_|64).unwrap();
        state=machines::MachineState::decode(&u.state.encode()).unwrap();
        if u.result==machines::TickResult::Completed {
            let remaining=restore(&[u.remaining],&[Some(input)]).unwrap();
            assert_eq!(remaining[0].as_ref().unwrap().count,1);
            assert_eq!(portable(&remaining)[0].as_ref().unwrap().components,original[0].as_ref().unwrap().components);
            let output=restore(&u.outputs.into_iter().map(Some).collect::<Vec<_>>(),&[]).unwrap();
            assert_eq!(&*output[0].as_ref().unwrap().item,"minecraft:oak_planks");assert_eq!(output[0].as_ref().unwrap().count,6);return;
        }
    }
    panic!("host saw did not finish");
}
#[test]
fn loose_bucket_drain_returns_a_real_empty_bucket_and_saved_fluid() {
    let input=horizon_client::item_data::parse_stack(r#"{id:"minecraft:water_bucket",count:1}"#).unwrap().unwrap();
    let stack=portable(&[Some(input)]);
    let mut state=machines::MachineState::default();
    for _ in 0..100 {
        let u=machines::surface::work(&state,93,stack[0].as_ref().unwrap(),"emptying",32.0,machines::Heat::None,recipes::active_recipes(),|_|64).unwrap();
        state=machines::MachineState::decode(&u.state.encode()).unwrap();
        if u.result==machines::TickResult::Completed {
            assert!(u.remaining.is_none());
            let output=restore(&u.outputs.into_iter().map(Some).collect::<Vec<_>>(),&[]).unwrap();
            assert_eq!(&*output[0].as_ref().unwrap().item,"minecraft:bucket");
            assert_eq!(state.fluid_out.get("minecraft:water"),Some(&1000));return;
        }
    }
    panic!("host drain did not finish");
}

#[test]
fn chute_world_capture_and_discharge_conserve_named_host_items() {
    let input=named_iron();let original=portable(&[Some(input.clone())]);
    let (capture,stored)=machines::transfer(&original,&vec![None;27],64,|_|64).unwrap();
    assert!(capture.after[0].is_none());
    let mut slots=restore(&stored.after,&[Some(input)]).unwrap();
    let mut outputs=Vec::new();
    for _ in 0..3 {
        let (left,out)=machines::transfer(&portable(&slots),&[None],1,|_|64).unwrap();
        outputs.extend(restore(&out.after,&slots).unwrap().into_iter().flatten());
        slots=restore(&left.after,&slots).unwrap();
    }
    assert!(slots.iter().all(Option::is_none));assert_eq!(outputs.iter().map(|s|s.count).sum::<u32>(),3);
    assert!(portable(&outputs.into_iter().map(Some).collect::<Vec<_>>()).iter().flatten().all(|s|s.components==original[0].as_ref().unwrap().components));
}
