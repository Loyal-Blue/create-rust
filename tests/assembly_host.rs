// Production item metadata/controller checks in the isolated host model.
fn assembly_seed(step: u8) -> ItemStack {
    let base = ItemStack::new(machines::assembly::BASE, 1);
    if step == 0 {
        return base;
    }
    let mut p = machines::assembly::Progress::start();
    p.completed = step;
    assembly_output(&base, machines::assembly::Outcome::Progress(p)).unwrap()
}
fn assembly_scene(step: u8, material: &str) -> Game<'static> {
    let mut g = scene(true);
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[0] = Some(assembly_seed(step));
    g.world.block_entities.get_mut(&(0, 67, 0)).unwrap().0[0] = Some(ItemStack::new(material, 1));
    g
}
fn assembly_catalog() -> ApplicationCatalog<'static> {
    ApplicationCatalog {
        regular: recipes::active_recipes(),
        assembly: true,
    }
}
#[test]
fn precision_assembly_completes_all_fifteen_operations_with_item_roundtrips() {
    let mut input = assembly_seed(0);
    let mut used = BTreeMap::new();
    for n in 0..15 {
        let mut p = machines::assembly::Progress::start();
        p.completed = n;
        let tool = p.tool().unwrap();
        let mut g = assembly_scene(n, tool);
        g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[0] = Some(input);
        ticks(&mut g, 100, 32.);
        let slots = &g.world.block_entities[&(0, 65, 0)].0;
        assert!(slots[0].is_none());
        assert!(g.world.block_entities[&(0, 67, 0)].0[0].is_none());
        *used.entry(tool).or_insert(0) += 1;
        input = horizon_items::item_nbt::item_from_nbt(&horizon_items::item_nbt::item_to_nbt(
            slots[9].as_ref().unwrap(),
        ))
        .unwrap();
        if n < 14 {
            assert_eq!(assembly_progress(&input).unwrap().unwrap().completed, n + 1);
        } else {
            assert!(
                machines::assembly::RESULTS
                    .iter()
                    .any(|(id, _)| *id == &*input.item)
            );
            assert!(
                assembly_custom_data(&input)
                    .unwrap()
                    .get(&ASSEMBLY_KEY.into())
                    .is_none()
            );
        }
    }
    assert_eq!(used["create:cogwheel"], 5);
    assert_eq!(used["create:large_cogwheel"], 5);
    assert_eq!(used["minecraft:iron_nugget"], 5);
}
#[test]
fn wrong_phase_material_and_blank_intermediates_are_preserved() {
    let mut g = assembly_scene(1, "create:cogwheel");
    let before = g.world.block_entities[&(0, 65, 0)].0.clone();
    ticks(&mut g, 200, 32.);
    assert_eq!(g.world.block_entities[&(0, 65, 0)].0, before);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        1
    );
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[0] =
        Some(ItemStack::new(machines::assembly::INTERMEDIATE, 1));
    ticks(&mut g, 200, 32.);
    assert_eq!(
        g.world.block_entities[&(0, 65, 0)].0[0]
            .as_ref()
            .unwrap()
            .item
            .as_ref(),
        machines::assembly::INTERMEDIATE
    );
    assert!(g.world.block_entities[&(0, 65, 0)].0[9].is_none());
}
#[test]
fn bulk_gold_is_split_into_individually_tracked_intermediates() {
    let mut g = scene(false);
    g.dropped[0].stack = ItemStack::new(machines::assembly::BASE, 3);
    g.world.block_entities.get_mut(&(0, 67, 0)).unwrap().0[0] =
        Some(ItemStack::new("create:cogwheel", 3));
    ticks(&mut g, 250, 32.);
    assert_eq!(count(&g, machines::assembly::INTERMEDIATE), 3);
    assert!(g.world.block_entities[&(0, 67, 0)].0[0].is_none());
    for e in &g.dropped {
        assert_eq!(assembly_progress(&e.stack).unwrap().unwrap().completed, 1);
    }
}
#[test]
fn blocked_final_result_rolls_back_rng_and_both_inputs() {
    let mut g = assembly_scene(14, "minecraft:iron_nugget");
    for slot in &mut g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[9..18] {
        *slot = Some(ItemStack::new("minecraft:dirt", 64));
    }
    let initial = read_machine(&g, (0, 67, 0));
    ticks(&mut g, 200, 32.);
    let blocked = read_machine(&g, (0, 67, 0));
    assert_eq!(blocked.rng, initial.rng);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        1
    );
    let input = g.world.block_entities[&(0, 65, 0)].0[0].clone().unwrap();
    assert_eq!(assembly_progress(&input).unwrap().unwrap().completed, 14);
    let hand = portable(&g.world.block_entities[&(0, 67, 0)].0);
    let expected = application_work(
        &blocked,
        machines::applying::Target {
            entity: 0,
            position: [0, 65, 0],
        },
        &input,
        &hand,
        32.,
        &assembly_catalog(),
        |_| 64,
    )
    .unwrap()
    .unwrap()
    .outputs[0]
        .clone();
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[9] = None;
    ticks(&mut g, 100, 32.);
    let actual = portable(&g.world.block_entities[&(0, 65, 0)].0);
    assert_eq!(actual[9], Some(expected));
    assert!(actual[0].is_none());
    assert!(g.world.block_entities[&(0, 67, 0)].0[0].is_none());
}
#[test]
fn assembly_preserves_unrelated_components_and_cleans_its_own_tag() {
    let base=horizon_client::item_data::parse_stack(r#"{id:"create:golden_sheet",count:1,components:{"minecraft:custom_name":{text:"Personal mechanism"},"minecraft:custom_data":{foreign:{nested:42},flag:1},"minecraft:lore":[{text:"Keep this lore"}]}}"#).unwrap().unwrap();
    let next = assembly_output(
        &base,
        machines::assembly::Outcome::Progress(machines::assembly::Progress {
            completed: 7,
            ..machines::assembly::Progress::start()
        }),
    )
    .unwrap();
    let value = portable(&[Some(next)])[0].clone().unwrap();
    let next = restore(&[Some(value)], &[]).unwrap()[0].clone().unwrap();
    assert_eq!(assembly_progress(&next).unwrap().unwrap().completed, 7);
    let finished = assembly_output(
        &next,
        machines::assembly::Outcome::Finished(machines::assembly::RESULT),
    )
    .unwrap();
    assert_eq!(
        finished.data().unwrap().custom_name,
        base.data().unwrap().custom_name
    );
    assert_eq!(
        assembly_custom_data(&finished).unwrap(),
        assembly_custom_data(&base).unwrap()
    );
    assert_eq!(
        portable(&[Some(finished)])[0].as_ref().unwrap().components,
        portable(&[Some(base)])[0].as_ref().unwrap().components
    );
}
#[test]
fn malformed_or_future_assembly_tags_never_consume_material() {
    let mut g = assembly_scene(1, "create:large_cogwheel");
    let mut bad = assembly_seed(1);
    bad.data_mut().other=vec![(CUSTOM_DATA.into(),AssemblyTag::compound([(ASSEMBLY_KEY,AssemblyTag::String("{\"version\":2,\"recipe\":\"create:sequenced_assembly/precision_mechanism\",\"completed\":14}".into()))]))];
    assert!(assembly_progress(&bad).is_err());
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[0] = Some(bad.clone());
    ticks(&mut g, 100, 32.);
    assert_eq!(g.world.block_entities[&(0, 65, 0)].0[0], Some(bad));
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        1
    );
}
#[test]
fn missing_assembly_registry_support_disables_processing() {
    let input = assembly_seed(0);
    let mut hand = vec![None; 27];
    hand[0] = Some(machines::Stack {
        id: "create:cogwheel".into(),
        count: 1,
        components: String::new(),
    });
    let no_registry = ApplicationCatalog {
        regular: recipes::active_recipes(),
        assembly: false,
    };
    assert!(!application_select(&input, &hand, &no_registry));
    assert!(
        application_work(
            &machines::MachineState::default(),
            machines::applying::Target {
                entity: 7,
                position: [0, 65, 0]
            },
            &input,
            &hand,
            32.,
            &no_registry,
            |_| 64
        )
        .unwrap()
        .is_none()
    );
}
#[test]
fn zero_power_does_not_advance_the_item_phase() {
    let mut g = assembly_scene(8, "minecraft:iron_nugget");
    let before = g.world.block_entities[&(0, 65, 0)].0[0].clone();
    ticks(&mut g, 100, 0.);
    assert_eq!(g.world.block_entities[&(0, 65, 0)].0[0], before);
    assert!(g.world.block_entities[&(0, 65, 0)].0[9].is_none());
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        1
    );
}
#[test]
fn fifteen_deployer_belt_line_finishes_without_manual_item_edits() {
    let mut g = scene(false);
    g.world.block_entities.clear();
    g.world.names.clear();
    g.dropped[0].stack = assembly_seed(0);
    for x in 0..17 {
        g.world.names.insert((x, 65, 0), "create:belt".into());
    }
    for x in 0..15 {
        let mut p = machines::assembly::Progress::start();
        p.completed = x as u8;
        g.world.names.insert((x, 67, 0), "create:deployer".into());
        let mut slots = vec![None; 27];
        slots[0] = Some(ItemStack::new(p.tool().unwrap(), 1));
        g.world.block_entities.insert((x, 67, 0), TestChest(slots));
    }
    for _ in 0..4000 {
        g.world.game_time += 1;
        let mut run = SurfaceRun::default();
        for x in 0..15 {
            tick_deployer_application(&mut g, (x, 67, 0), 32., &mut run);
        }
        for e in &mut g.dropped {
            if !run.held.contains(&e.id) {
                e.position.x += 32. / 512.;
            }
        }
        if g.dropped[0].position.x > 16.5 {
            break;
        }
    }
    assert_eq!(g.dropped.len(), 1);
    assert!(
        machines::assembly::RESULTS
            .iter()
            .any(|(id, _)| *id == &*g.dropped[0].stack.item)
    );
    assert!(
        assembly_custom_data(&g.dropped[0].stack)
            .unwrap()
            .get(&ASSEMBLY_KEY.into())
            .is_none()
    );
    for x in 0..15 {
        assert!(
            g.world.block_entities[&(x, 67, 0)].0[0].is_none(),
            "station {x} skipped"
        );
    }
}
