// Straight, horizontal transport over the engine's existing item entities.
use machines::transport::{self, Facing as BeltFacing};

fn transport_facing(game: &Game<'_>, p: MachinePos) -> Option<BeltFacing> {
    let state = game
        .pack
        .assets
        .registry
        .state(game.world.block(p.0, p.1, p.2))?;
    BeltFacing::parse(state.properties.get("facing")?)
}
fn offset_machine(p: MachinePos, d: [i32; 3], n: i32) -> MachinePos {
    (p.0 + d[0] * n, p.1 + d[1] * n, p.2 + d[2] * n)
}
fn transport_notice(game: &Game<'_>, p: MachinePos, text: &str) {
    broadcast_machine_state(
        game,
        &[MachineRecord {
            pos: [p.0, p.1, p.2],
            name: machine_name(game, p),
            data: read_machine(game, p),
        }],
        text,
    );
}
pub(super) fn use_transport(game: &mut Game<'_>, p: [i32; 3]) -> Result<bool> {
    if game.remote.is_some() || !game.may_build() {
        return Ok(false);
    }
    let pos = (p[0], p[1], p[2]);
    let held = game.held().cloned();
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    if held
        .as_ref()
        .is_some_and(|s| &*s.item == "create:belt_connector")
    {
        type Key = (std::path::PathBuf, String, Option<u128>);
        thread_local! {static STARTS:std::cell::RefCell<BTreeMap<Key,([i32;3],std::time::Instant)>>=const{std::cell::RefCell::new(BTreeMap::new())};}
        let key = (
            game.save.root.clone(),
            format!("{:?}", game.dim),
            game.lan_acting_uuid,
        );
        if sneaking {
            STARTS.with(|v| v.borrow_mut().remove(&key));
            transport_notice(game, pos, "Create belt connection cancelled");
            return Ok(true);
        }
        let Some(state) = game
            .pack
            .assets
            .registry
            .state(game.world.block(p[0], p[1], p[2]))
        else {
            return Ok(true);
        };
        if state.name != "create:shaft"
            || state
                .properties
                .get("waterlogged")
                .is_some_and(|v| v == "true")
        {
            transport_notice(
                game,
                pos,
                "Belt connector: select two dry horizontal shafts",
            );
            return Ok(true);
        }
        let axis = match read_axis(state) {
            Axis::X => transmission::Axis::X,
            Axis::Z => transmission::Axis::Z,
            _ => {
                transport_notice(game, pos, "Belt connector: shaft must be horizontal");
                return Ok(true);
            }
        };
        let start = STARTS
            .with(|v| v.borrow_mut().remove(&key))
            .filter(|(_, t)| t.elapsed().as_secs() < 120)
            .map(|(p, _)| p);
        let Some(start) = start else {
            STARTS.with(|v| {
                v.borrow_mut()
                    .retain(|_, (_, t)| t.elapsed().as_secs() < 120);
                v.borrow_mut().insert(key, (p, std::time::Instant::now()));
            });
            transport_notice(
                game,
                pos,
                "Belt start selected. Select a parallel shaft within 16 blocks",
            );
            return Ok(true);
        };
        let Some((facing, plan)) = transport::connection(start, p, axis) else {
            transport_notice(
                game,
                pos,
                "Belts need a straight, level run perpendicular to both shafts (max 16 blocks)",
            );
            return Ok(true);
        };
        let mut originals = Vec::new();
        for (i, q) in plan.iter().enumerate() {
            if !game.world.loaded(q[0], q[2]) {
                transport_notice(game, pos, "Belt path crosses an unloaded chunk");
                return Ok(true);
            }
            let id = game.world.block(q[0], q[1], q[2]);
            if i == 0 || i + 1 == plan.len() {
                let valid = game.pack.assets.registry.state(id).is_some_and(|s| {
                    s.name == "create:shaft"
                        && s.properties.get("axis").is_some_and(|v| {
                            v == axis_name(match axis {
                                transmission::Axis::X => Axis::X,
                                _ => Axis::Z,
                            })
                        })
                        && !s.properties.get("waterlogged").is_some_and(|v| v == "true")
                });
                if !valid {
                    transport_notice(game, pos, "Belt endpoints changed; select the shafts again");
                    return Ok(true);
                }
            } else if !game.res.facts().is_air(id) {
                transport_notice(game, pos, "Belt path is blocked; no blocks were replaced");
                return Ok(true);
            }
            originals.push(id);
        }
        let Some(belt) = game
            .pack
            .assets
            .registry
            .default_state("create:belt")
            .and_then(|id| game.with_props(id, &[("facing", facing.name())]))
        else {
            return Ok(true);
        };
        for (i, q) in plan.iter().enumerate() {
            if let Err(error) = game.level_set(*q, belt) {
                for (q, old) in plan[..=i].iter().zip(&originals[..=i]) {
                    game.level_set(*q, *old)?;
                }
                return Err(error);
            }
        }
        game.consume_held();
        transport_notice(
            game,
            pos,
            "Create belt connected. Power either end shaft from the side",
        );
        return Ok(true);
    }
    let name = machine_name(game, pos);
    if !matches!(
        name.as_str(),
        "create:andesite_funnel" | "create:brass_funnel"
    ) || held.as_ref().is_some_and(|s| &*s.item == "create:wrench")
    {
        return Ok(false);
    }
    if sneaking && held.is_some() {
        return Ok(false);
    }
    let mut state = read_machine(game, pos);
    if let Some(held) = held {
        if name != "create:brass_funnel" {
            return Ok(false);
        }
        state.item_filter = Some(held.item.to_string());
    } else if sneaking {
        if state.item_filter.is_some() {
            state.item_filter = None;
        } else {
            state.transfer_limit = match state.transfer_limit {
                1 => 16,
                16 => 64,
                _ => 1,
            };
        }
    } else {
        state.extracting = !state.extracting;
    }
    let text = format!(
        "Funnel {} | {} items | filter {}",
        if state.extracting {
            "extract"
        } else {
            "insert"
        },
        state.transfer_limit,
        state.item_filter.as_deref().unwrap_or("any")
    );
    store_machine(game, pos, state);
    transport_notice(game, pos, &text);
    Ok(true)
}

fn funnel_export(
    game: &mut Game<'_>,
    p: MachinePos,
    facing: BeltFacing,
    state: &machines::MachineState,
) {
    let step = facing.step();
    let container = offset_machine(p, step, -1);
    let front = offset_machine(p, step, 1);
    if !matches!(machine_name(game, front).as_str(), "create:belt")
        || !game.world.loaded(container.0, container.2)
    {
        return;
    }
    let name = machine_name(game, container);
    if !transport::supported_container(&name) {
        return;
    }
    // Backpressure: a blocked receiving surface must not accumulate a new stack every tick.
    let mouth = [
        p.0 as f64 + 0.5 + f64::from(step[0]) * 0.85,
        p.1 as f64 + transport::BELT_SURFACE + 0.02,
        p.2 as f64 + 0.5 + f64::from(step[2]) * 0.85,
    ];
    if game.dropped.iter().any(|e| {
        transport::at_mouth(
            facing,
            [
                e.position.x - (p.0 as f64 + 0.5),
                e.position.y - (p.1 as f64 + transport::BELT_SURFACE),
                e.position.z - (p.2 as f64 + 0.5),
            ],
        )
    }) {
        return;
    }
    let Some(entity) = game.world.block_entities.get(&container) else {
        return;
    };
    let before = entity.slots().to_vec();
    let range = transport::source_slots(&name, before.len());
    let portable_before = portable(&before);
    let Some(index) = range.into_iter().find(|&i| {
        portable_before[i]
            .as_ref()
            .is_some_and(|s| transport::filter_accepts(state.item_filter.as_deref(), s))
    }) else {
        return;
    };
    let Some((source, dest)) = machines::transfer(
        &[portable_before[index].clone()],
        &[None],
        state.transfer_limit,
        |id| game.items().get(id).map_or(1, |i| i.max_stack),
    ) else {
        return;
    };
    let (Some(left), Some(taken)) = (
        restore(&source.after, &before),
        restore(&dest.after, &before),
    ) else {
        return;
    };
    let Some(stack) = taken.into_iter().next().flatten() else {
        return;
    };
    if let Some(entity) = game.world.block_entities.get_mut(&container) {
        if entity.slots() != before {
            return;
        }
        entity.slots_mut()[index] = left.into_iter().next().flatten();
    } else {
        return;
    }
    dirty_machine(game, container);
    game.spawn_item(stack, mouth, [0.0, 0.0, 0.0], 10);
}
fn funnel_import(
    game: &mut Game<'_>,
    p: MachinePos,
    facing: BeltFacing,
    state: &machines::MachineState,
) {
    let step = facing.step();
    let container = offset_machine(p, step, -1);
    let name = machine_name(game, container);
    if !game.world.loaded(container.0, container.2) || !transport::supported_container(&name) {
        return;
    }
    let Some(entity) = game.world.block_entities.get(&container) else {
        return;
    };
    let before = entity.slots().to_vec();
    let range = transport::input_slots(&name, before.len());
    if range.is_empty() {
        return;
    }
    for i in 0..game.dropped.len() {
        let item = &game.dropped[i];
        if item.target.is_some()
            || !transport::at_mouth(
                facing,
                [
                    item.position.x - (p.0 as f64 + 0.5),
                    item.position.y - (p.1 as f64 + transport::BELT_SURFACE),
                    item.position.z - (p.2 as f64 + 0.5),
                ],
            )
        {
            continue;
        }
        let original = item.stack.clone();
        let source = portable(&[Some(original.clone())]);
        if !transport::filter_accepts(state.item_filter.as_deref(), source[0].as_ref().unwrap()) {
            continue;
        }
        let Some((source, dest)) = machines::transfer(
            &source,
            &portable(&before[range.clone()]),
            state.transfer_limit,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            continue;
        };
        let mut originals = before.clone();
        originals.push(Some(original.clone()));
        let (Some(left), Some(after)) = (
            restore(&source.after, &originals),
            restore(&dest.after, &originals),
        ) else {
            continue;
        };
        if let Some(entity) = game.world.block_entities.get_mut(&container) {
            if entity.slots() != before {
                return;
            }
            entity.slots_mut()[range].clone_from_slice(&after);
        } else {
            return;
        }
        if let Some(left) = left.into_iter().next().flatten() {
            game.dropped[i].stack = left;
        } else {
            game.dropped.remove(i);
        }
        dirty_machine(game, container);
        return;
    }
}
fn tick_transport(
    game: &mut Game<'_>,
    positions: &[(MachinePos, String)],
    speeds: &std::collections::HashMap<[i32; 3], f32>,
) {
    let mut belts = BTreeMap::new();
    for (p, name) in positions {
        let Some(facing) = transport_facing(game, *p) else {
            continue;
        };
        if name == "create:belt" {
            belts.insert(
                *p,
                (facing, speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0)),
            );
        }
        if matches!(
            name.as_str(),
            "create:andesite_funnel" | "create:brass_funnel"
        ) {
            let mut state = read_machine(game, *p);
            let powered = game
                .with_level(|b, l, t| b.has_neighbor_signal(l, t, BlockPos::new(p.0, p.1, p.2)));
            state.status = if powered {
                "Paused"
            } else if state.extracting {
                "Extracting"
            } else {
                "Inserting"
            }
            .into();
            store_machine(game, *p, state.clone());
            if !powered && game.world.game_time.rem_euclid(8) == 0 {
                if state.extracting {
                    funnel_export(game, *p, facing, &state);
                } else {
                    funnel_import(game, *p, facing, &state);
                }
            }
        }
    }
    for item in &mut game.dropped {
        let p = (
            item.position.x.floor() as i32,
            item.position.y.floor() as i32,
            item.position.z.floor() as i32,
        );
        let Some((facing, rpm)) = belts.get(&p) else {
            continue;
        };
        if let Some([x, z]) = transport::belt_velocity(
            *facing,
            *rpm,
            [
                item.position.x - f64::from(p.0),
                item.position.y - f64::from(p.1),
                item.position.z - f64::from(p.2),
            ],
        ) {
            item.velocity.x = x;
            item.velocity.z = z;
            if item.target.is_none() {
                item.age = 0;
            }
        }
    }
}
