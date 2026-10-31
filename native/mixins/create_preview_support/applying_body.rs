// Two inventories or a held inventory + live entity commit in one server tick.
fn tick_deployer_application(
    game: &mut Game<'_>,
    p: MachinePos,
    rpm: f32,
    run: &mut SurfaceRun,
) -> bool {
    if game.with_level(|b, l, t| b.has_neighbor_signal(l, t, BlockPos::new(p.0, p.1, p.2))) {
        let mut state = read_machine(game, p);
        state.status = "Redstone paused".into();
        store_machine(game, p, state);
        return true;
    }
    let Some(entity) = game.world.block_entities.get(&p) else {
        return false;
    };
    let hand_before = entity.slots().to_vec();
    let hand = portable(&hand_before);
    let mut regular = surface_catalog(game, "deploying");
    regular.extend(surface_catalog(game, "item_application"));
    // A complete recipe in the machine's own inventory keeps its old priority.
    let mut stock = recipes::Stock {
        slots: usize::MAX,
        ..Default::default()
    };
    for s in hand.iter().take(9).flatten() {
        *stock.items.entry(s.id.clone()).or_default() += s.count;
    }
    if recipes::matching_recipe(&stock, "deploying", &regular).is_some() {
        clear_world_work(game, p);
        return false;
    }
    let catalog = ApplicationCatalog {
        regular: &regular,
        assembly: std::iter::once(machines::assembly::INTERMEDIATE)
            .chain(machines::assembly::RESULTS.iter().map(|(id, _)| *id))
            .all(|id| game.items().get(id).is_some()),
    };
    let step = facing_step(game, p);
    let Some(target) = machines::placing::target([p.0, p.1, p.2], step) else {
        return false;
    };
    let middle = offset_machine(p, step, 1);
    if !game.world.loaded(middle.0, middle.2)
        || !game.world.loaded(target[0], target[2])
        || game
            .res
            .facts()
            .get(game.world.block(middle.0, middle.1, middle.2))
            .full_collision
    {
        clear_world_work(game, p);
        return false;
    }
    let cell = (target[0], target[1], target[2]);
    let cell_name = machine_name(game, cell);
    if !matches!(cell_name.as_str(), "create:belt" | "create:depot")
        && !game
            .res
            .facts()
            .is_air(game.world.block(cell.0, cell.1, cell.2))
    {
        clear_world_work(game, p);
        return false;
    }
    let surfaces = [cell, (cell.0, cell.1 - 1, cell.2)];
    let mut state = read_machine(game, p);
    state.set_speed(rpm, game.world.game_time.max(0) as u64);
    let previous = state.applied_entities.len();
    state.applied_entities.retain(|binding| {
        game.dropped.iter().any(|item| {
            item.id == binding.entity
                && (item.position.x - (cell.0 as f64 + 0.5)).abs() <= 0.48
                && (item.position.z - (cell.2 as f64 + 0.5)).abs() <= 0.48
                && (item.position.y - cell.1 as f64).abs() <= 1.25
                && portable(&[Some(item.stack.clone())])[0]
                    .as_ref()
                    .is_some_and(|s| surface::material(s) == binding.material)
        })
    });
    if state.applied_entities.len() != previous {
        store_machine(game, p, state.clone());
    }
    if state.applied_entities.len() > 119 {
        state.status = "Clear deployer outputs".into();
        store_machine(game, p, state);
        return true;
    }

    for q in surfaces {
        if machine_name(game, q) != "create:depot" {
            continue;
        }
        let Some(e) = game.world.block_entities.get(&q) else {
            continue;
        };
        let before = e.slots().to_vec();
        let slots = portable(&before);
        let Some(slot) = before.iter().take(9).position(|s| {
            s.as_ref()
                .is_some_and(|s| application_select(s, &hand, &catalog))
        }) else {
            continue;
        };
        let Ok(Some(update)) = application_work(
            &state,
            machines::applying::Target {
                entity: 0,
                position: [q.0, q.1, q.2],
            },
            before[slot].as_ref().unwrap(),
            &hand,
            rpm,
            &catalog,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            continue;
        };
        let Ok((mut update, plan)) = update.depot(&slots, slot, |id| {
            game.items().get(id).map_or(1, |i| i.max_stack)
        }) else {
            continue;
        };
        let mut originals = hand_before.clone();
        originals.extend(before.clone());
        let (Some(after_hand), Some(after_depot)) = (
            restore(&update.hand.after, &originals),
            restore(&plan.after, &originals),
        ) else {
            continue;
        };
        if game
            .world
            .block_entities
            .get(&p)
            .is_none_or(|e| e.slots() != hand_before)
            || game
                .world
                .block_entities
                .get(&q)
                .is_none_or(|e| e.slots() != before)
        {
            return true;
        }
        game.world
            .block_entities
            .get_mut(&p)
            .unwrap()
            .slots_mut()
            .clone_from_slice(&after_hand);
        game.world
            .block_entities
            .get_mut(&q)
            .unwrap()
            .slots_mut()
            .clone_from_slice(&after_depot);
        if update.result == machines::TickResult::NoPower
            && matches!(state.status.as_str(), "Overstressed" | "Conflict")
        {
            update.state.status = state.status.clone();
        }
        dirty_machine(game, p);
        dirty_machine(game, q);
        store_machine(game, p, update.state);
        return true;
    }
    let candidate = game
        .dropped
        .iter()
        .enumerate()
        .find(|(_, item)| {
            if item.target.is_some() || run.held.contains(&item.id) || !item.on_ground {
                return false;
            }
            let support = (
                item.position.x.floor() as i32,
                (item.position.y - 0.02).floor() as i32,
                item.position.z.floor() as i32,
            );
            if !surfaces.contains(&support)
                || !matches!(
                    machine_name(game, support).as_str(),
                    "create:belt" | "create:depot"
                )
                || (item.position.x - (cell.0 as f64 + 0.5)).abs() > 0.48
                || (item.position.z - (cell.2 as f64 + 0.5)).abs() > 0.48
            {
                return false;
            }
            portable(&[Some(item.stack.clone())])[0]
                .as_ref()
                .is_some_and(|s| {
                    !machines::applying::applied(&state, item.id, s)
                        && application_select(&item.stack, &hand, &catalog)
                })
        })
        .map(|(i, e)| (i, e.id, e.stack.clone()));
    let Some((index, id, before)) = candidate else {
        clear_world_work(game, p);
        return false;
    };
    let Ok(Some(mut update)) = application_work(
        &state,
        machines::applying::Target {
            entity: id,
            position: target,
        },
        &before,
        &hand,
        rpm,
        &catalog,
        |id| game.items().get(id).map_or(1, |i| i.max_stack),
    ) else {
        return false;
    };
    let mut originals = hand_before.clone();
    originals.push(Some(before.clone()));
    let Some(after_hand) = restore(&update.hand.after, &originals) else {
        return true;
    };
    if game
        .world
        .block_entities
        .get(&p)
        .is_none_or(|e| e.slots() != hand_before)
    {
        return true;
    }
    let world = surface::WorldUpdate {
        state: update.state.clone(),
        before: update.before.clone(),
        remaining: update.remaining.clone(),
        outputs: update.outputs.clone(),
        result: update.result,
    };
    let last_entity = game.next_entity;
    if !commit_surface_stack(game, index, id, &before, &world) {
        return true;
    }
    game.world
        .block_entities
        .get_mut(&p)
        .unwrap()
        .slots_mut()
        .clone_from_slice(&after_hand);
    if update.result == machines::TickResult::Completed {
        for item in &game.dropped {
            if item.id > last_entity || (item.id == id && update.remaining.is_none()) {
                if let Some(stack) = portable(&[Some(item.stack.clone())])[0].as_ref() {
                    let recorded = machines::applying::remember(&mut update.state, item.id, stack);
                    debug_assert!(recorded);
                }
            }
        }
    }

    if rpm != 0.
        && update.remaining.is_some()
        && game
            .dropped
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|e| application_select(&e.stack, &update.hand.after, &catalog))
    {
        run.held.insert(id);
        if let Some(item) = game.dropped.iter_mut().find(|e| e.id == id) {
            item.position.x = cell.0 as f64 + 0.5;
            item.position.z = cell.2 as f64 + 0.5;
            item.velocity.x = 0.;
            item.velocity.z = 0.;
        }
    }
    if update.result == machines::TickResult::NoPower
        && matches!(state.status.as_str(), "Overstressed" | "Conflict")
    {
        update.state.status = state.status;
    }
    dirty_machine(game, p);
    store_machine(game, p, update.state);
    true
}
