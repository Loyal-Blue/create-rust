// Production item/component plans validate every seeded assembly attempt.
use std::collections::BTreeMap;
fn point(v: &serde_json::Value) -> [i32; 3] {
    serde_json::from_value(v.clone()).unwrap()
}
pub fn verify(plan: &serde_json::Value) -> std::result::Result<(), String> {
    let blocks: BTreeMap<_, _> = plan["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (point(&v["pos"]), v))
        .collect();
    let containers: BTreeMap<_, _> = plan["containers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (point(&v["pos"]), v))
        .collect();
    let catalog = ApplicationCatalog {
        regular: recipes::active_recipes(),
        assembly: true,
    };
    for case in plan["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("assembly").is_some())
    {
        let line = &case["assembly"];
        let controllers: Vec<_> = line["controllers"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect();
        if controllers.len() != 15 {
            return Err("Precision line needs 15 controllers".into());
        }
        let count = line["count"].as_u64().unwrap() as u32;
        let chest = point(&line["input"]);
        let source = &containers[&chest]["items"][0];
        if source["id"] != machines::assembly::BASE || source["count"] != count {
            return Err("Assembly feed mismatch".into());
        }
        let mut hands = vec![];
        let mut states = vec![machines::MachineState::default(); 15];
        for (n, q) in controllers.iter().enumerate() {
            if blocks[q]["name"] != "create:deployer" || blocks[q]["properties"]["facing"] != "down"
            {
                return Err("Invalid assembly controller".into());
            }
            let surface = [q[0], q[1] - 2, q[2]];
            if blocks[&surface]["name"] != "create:belt" {
                return Err("Assembly needs receiving belts".into());
            }
            if n > 0
                && *q
                    != [
                        controllers[n - 1][0] + 1,
                        controllers[n - 1][1],
                        controllers[n - 1][2],
                    ]
            {
                return Err("Assembly line must be continuous".into());
            }
            let mut p = machines::assembly::Progress::start();
            p.completed = n as u8;
            let seed = &containers[q]["items"][0];
            if seed["id"] != p.tool().unwrap() || seed["count"] != count {
                return Err("Assembly held material mismatch".into());
            }
            let mut slots = vec![None; 27];
            slots[0] = Some(ItemStack::new(p.tool().unwrap(), count));
            hands.push(slots);
        }
        let mut products: BTreeMap<String, u32> = BTreeMap::new();
        for entity in 1..=count {
            let mut input = ItemStack::new(machines::assembly::BASE, 1);
            for (n, &q) in controllers.iter().enumerate() {
                let mut complete = false;
                for _ in 0..1000 {
                    let before = hands[n].clone();
                    let hand = portable(&before);
                    if !application_select(&input, &hand, &catalog) {
                        return Err("Assembly selection failed".into());
                    }
                    let u = application_work(
                        &states[n],
                        machines::applying::Target {
                            entity,
                            position: [q[0], q[1] - 2, q[2]],
                        },
                        &input,
                        &hand,
                        32.,
                        &catalog,
                        |id| {
                            if id == machines::assembly::INTERMEDIATE {
                                1
                            } else {
                                64
                            }
                        },
                    )?
                    .ok_or("Assembly step did not match")?;
                    states[n] = machines::MachineState::decode(&u.state.encode())?;
                    let mut originals = before.clone();
                    originals.push(Some(input.clone()));
                    hands[n] = restore(&u.hand.after, &originals).ok_or("Held restore failed")?;
                    if u.result == machines::TickResult::Completed {
                        let outputs = restore(
                            &u.outputs.into_iter().map(Some).collect::<Vec<_>>(),
                            &originals,
                        )
                        .ok_or("Assembly output restore failed")?;
                        if outputs.len() != 1 {
                            return Err("Invalid assembly output count".into());
                        }
                        input = outputs[0].clone().ok_or("Missing output")?;
                        let nbt = horizon_client::save::encode_block_entity(
                            (q[0], q[1], q[2]),
                            &horizon_client::containers::BlockEntity::Chest(vec![Some(
                                input.clone(),
                            )]),
                            "minecraft:chest",
                        );
                        let (_, decoded) = horizon_client::save::decode_block_entity(&nbt)
                            .ok_or("Intermediate save failed")?;
                        input = decoded.slots()[0].clone().unwrap();
                        if n < 14
                            && assembly_progress(&input)?.ok_or("Missing phase")?.completed
                                != n as u8 + 1
                        {
                            return Err("Assembly phase lost".into());
                        }
                        complete = true;
                        break;
                    }
                }
                if !complete {
                    return Err("Assembly stalled".into());
                }
            }
            if !machines::assembly::RESULTS
                .iter()
                .any(|(id, _)| *id == input.item.as_ref())
                || assembly_custom_data(&input)?.contains_key(&ASSEMBLY_KEY.into())
            {
                return Err("Invalid final assembly item".into());
            }
            *products.entry(input.item.to_string()).or_default() += input.count;
        }
        if hands.iter().any(|s| s[0].is_some()) || products.values().sum::<u32>() != count {
            return Err("Assembly did not conserve inputs".into());
        }
        println!(
            "PASS: {count} full 15-operation assembly runs; production component/save codecs; five of each held material per input; outputs {products:?}"
        );
    }
    Ok(())
}
