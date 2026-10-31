// Visible processing uses live item entities or the depot's real inventory.
use machines::surface;
use std::collections::BTreeSet;
#[derive(Default)]
struct SurfaceRun {
    handled: BTreeSet<MachinePos>,
    held: BTreeSet<u32>,
}
fn clear_world_work(game: &mut Game<'_>, p: MachinePos) {
    let mut state = read_machine(game, p);
    if state.world_target.take().is_some() {
        state.progress = 0.0;
        state.recipe = None;
        store_machine(game, p, state);
    }
}
fn facing_step(game: &Game<'_>, p: MachinePos) -> [i32; 3] {
    match game
        .pack
        .assets
        .registry
        .state(game.world.block(p.0, p.1, p.2))
        .and_then(|s| s.properties.get("facing"))
        .map(String::as_str)
    {
        Some("north") => [0, 0, -1],
        Some("west") => [-1, 0, 0],
        Some("east") => [1, 0, 0],
        Some("up") => [0, 1, 0],
        Some("down") => [0, -1, 0],
        _ => [0, 0, 1],
    }
}
fn surface_catalog(game: &Game<'_>, station: &str) -> Vec<recipes::Recipe> {
    recipes::active_recipes()
        .iter()
        .filter(|r| {
            r.station == station
                && r.products
                    .iter()
                    .all(|p| p.fluid || game.items().get(p.id).is_some())
        })
        .copied()
        .collect()
}
fn fan_reach(game: &Game<'_>, p: MachinePos, step: [i32; 3]) -> u32 {
    let mut reach = 0;
    for n in 1..=8 {
        let q = offset_machine(p, step, n);
        if !game.world.loaded(q.0, q.2) {
            break;
        }
        let state = game.world.block(q.0, q.1, q.2);
        if game.res.facts().get(state).full_collision {
            break;
        }
        reach = n as u32;
    }
    reach
}
fn fan_station(game: &Game<'_>, p: MachinePos, step: [i32; 3]) -> Option<&'static str> {
    let q = offset_machine(p, step, 1);
    let state = game
        .pack
        .assets
        .registry
        .state(game.world.block(q.0, q.1, q.2))?;
    match state.name.as_str() {
        "minecraft:water" => Some("splashing"),
        "minecraft:lava" => Some("blasting"),
        "minecraft:fire" => Some("smoking"),
        "minecraft:soul_fire" => Some("haunting"),
        "minecraft:campfire" if state.properties.get("lit").is_some_and(|v| v == "true") => {
            Some("smoking")
        }
        "minecraft:soul_campfire" if state.properties.get("lit").is_some_and(|v| v == "true") => {
            Some("haunting")
        }
        _ => None,
    }
}
fn has_machine_inputs(game: &Game<'_>, p: MachinePos) -> bool {
    game.world
        .block_entities
        .get(&p)
        .is_some_and(|e| machines::has_inputs(&portable(e.slots())))
}
fn tick_surface_machines(
    game: &mut Game<'_>,
    positions: &[(MachinePos, String)],
    speeds: &std::collections::HashMap<[i32; 3], f32>,
) -> SurfaceRun {
    let mut run = SurfaceRun::default();
    for (p, name) in positions {
        if !matches!(
            name.as_str(),
            "create:mechanical_press" | "create:spout" | "create:encased_fan"
        ) {
            continue;
        }
        let mut rpm = speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0);
        if name == "create:spout" {
            rpm = 32.0;
        }
        let step = facing_step(game, *p);
        let fan = name == "create:encased_fan";
        let reach = if fan { fan_reach(game, *p, step) } else { 0 };
        if fan && rpm != 0.0 {
            for item in &mut game.dropped {
                if surface::in_stream(
                    [p.0, p.1, p.2],
                    step,
                    reach,
                    [item.position.x, item.position.y, item.position.z],
                ) {
                    let force = (f64::from(rpm) / 1024.0).clamp(-0.15, 0.15);
                    item.velocity.x += (f64::from(step[0]) * force - item.velocity.x) * 0.25;
                    if step[1] != 0 {
                        item.velocity.y = f64::from(step[1]) * force + 0.04;
                    }
                    item.velocity.z += (f64::from(step[2]) * force - item.velocity.z) * 0.25;
                }
            }
        }
        if has_machine_inputs(game, *p) {
            clear_world_work(game, *p);
            continue;
        }
        let station = if fan {
            fan_station(game, *p, step)
        } else if name == "create:spout" {
            Some("filling")
        } else {
            Some("pressing")
        };
        let Some(station) = station else {
            clear_world_work(game, *p);
            continue;
        };
        let catalog = surface_catalog(game, station);
        let mut state = read_machine(game, *p);
        state.set_speed(rpm, game.world.game_time.max(0) as u64);
        // Depot contents take priority over loose stacks in the same work area.
        let depot = positions.iter().find(|(q, n)| {
            n == "create:depot"
                && has_machine_inputs(game, *q)
                && game.world.block_entities.get(q).is_some_and(|e| {
                    let owner = if name == "create:spout" {
                        &state
                    } else {
                        &read_machine(game, *q)
                    };
                    portable(e.slots())
                        .iter()
                        .take(machines::INPUT.end)
                        .flatten()
                        .any(|stack| surface::recipe(owner, stack, station, &catalog).is_some())
                })
                && if fan {
                    surface::in_stream(
                        [p.0, p.1, p.2],
                        step,
                        reach,
                        [
                            q.0 as f64 + 0.5,
                            q.1 as f64 + transport::BELT_SURFACE,
                            q.2 as f64 + 0.5,
                        ],
                    )
                } else {
                    q.0 == p.0 && q.2 == p.2 && (1..=2).contains(&(p.1 - q.1))
                }
        });
        if let Some((q, _)) = depot {
            if fan {
                let _ = process_with_fluid_owner(game, *q, *q, station, rpm, machines::Heat::None);
                let progress = read_machine(game, *q);
                let mut controller = state;
                controller.progress = progress.progress;
                controller.recipe = progress.recipe;
                controller.status = progress.status;
                controller.world_target = None;
                store_machine(game, *p, controller);
                run.handled.insert(*p);
            } else if name == "create:spout" {
                let _ = process_with_fluid_owner(game, *q, *p, station, rpm, machines::Heat::None);
                run.handled.insert(*p);
            }
            continue;
        }
        let mut choices: Vec<_> = game
            .dropped
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if item.target.is_some() || run.held.contains(&item.id) {
                    return false;
                }
                let at = [item.position.x, item.position.y, item.position.z];
                let inside = if fan {
                    surface::in_stream([p.0, p.1, p.2], step, reach, at)
                } else {
                    if !item.on_ground || !surface::below_controller([p.0, p.1, p.2], at) {
                        return false;
                    }
                    let support = (
                        at[0].floor() as i32,
                        (at[1] - 0.02).floor() as i32,
                        at[2].floor() as i32,
                    );
                    matches!(
                        machine_name(game, support).as_str(),
                        "create:belt" | "create:depot"
                    )
                };
                if !inside {
                    return false;
                }
                let stack = portable(&[Some(item.stack.clone())]);
                surface::recipe(&state, stack[0].as_ref().unwrap(), station, &catalog).is_some()
            })
            .map(|(i, item)| (i, item.id))
            .collect();
        choices.sort_by_key(|(_, id)| (!state.world_target.is_some_and(|b| b.entity == *id), *id));
        let Some(&(index, id)) = choices.first() else {
            clear_world_work(game, *p);
            continue;
        };
        let before = game.dropped[index].stack.clone();
        let input = portable(&[Some(before.clone())]);
        let Ok(mut update) = surface::work(
            &state,
            id,
            input[0].as_ref().unwrap(),
            station,
            rpm,
            machines::Heat::None,
            &catalog,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            continue;
        };
        if update.result == machines::TickResult::NoPower
            && matches!(state.status.as_str(), "Overstressed" | "Conflict")
        {
            update.state.status = state.status.clone();
        }
        if !commit_surface_stack(game, index, id, &before, &update) {
            continue;
        }
        let active =
            !fan && rpm != 0.0
                && update.remaining.as_ref().is_some_and(|s| {
                    surface::recipe(&update.state, s, station, &catalog).is_some()
                });
        if active {
            run.held.insert(id);
            if let Some(item) = game.dropped.iter_mut().find(|e| e.id == id) {
                item.position.x = p.0 as f64 + 0.5;
                item.position.z = p.2 as f64 + 0.5;
                item.velocity.x = 0.0;
                item.velocity.z = 0.0;
            }
        }
        store_machine(game, *p, update.state);
        run.handled.insert(*p);
    }
    tick_crushing_pairs(game, positions, speeds, &mut run);
    tick_top_processors(game, positions, speeds, &mut run);
    run
}

fn commit_surface_stack(
    game: &mut Game<'_>,
    index: usize,
    id: u32,
    before: &ItemStack,
    update: &surface::WorldUpdate,
) -> bool {
    if game
        .dropped
        .get(index)
        .is_none_or(|e| e.id != id || e.stack != *before)
    {
        return false;
    }
    let mut outputs = Vec::new();
    let mut valid = true;
    for output in &update.outputs {
        let Some(v) = restore(&[Some(output.clone())], &[Some(before.clone())])
            .and_then(|v| v.into_iter().next().flatten())
        else {
            valid = false;
            break;
        };
        outputs.push(v);
    }
    if !valid {
        return false;
    }
    let Some(remaining) = restore(&[update.remaining.clone()], &[Some(before.clone())]) else {
        return false;
    };
    let remaining = remaining.into_iter().next().flatten();
    let position = game.dropped[index].position;
    if update.result == machines::TickResult::Completed {
        if let Some(remaining) = remaining {
            game.dropped[index].stack = remaining;
        } else if !outputs.is_empty() {
            game.dropped[index].stack = outputs.remove(0);
        } else {
            game.dropped.remove(index);
        }
        for output in outputs {
            game.spawn_item(
                output,
                [position.x + 0.06, position.y + 0.03, position.z + 0.06],
                [0.0, 0.0, 0.0],
                10,
            );
        }
    }
    true
}

fn tick_depots(game: &mut Game<'_>, positions: &[(MachinePos, String)]) {
    for (p, name) in positions {
        if name != "create:depot" {
            continue;
        }
        // Take one nearby stack per tick; inventories stay the only stored copy.
        let candidate = game.dropped.iter().position(|e| {
            e.target.is_none()
                && e.on_ground
                && (e.position.x - (p.0 as f64 + 0.5)).abs() <= 0.45
                && (e.position.z - (p.2 as f64 + 0.5)).abs() <= 0.45
                && (e.position.y - (p.1 as f64 + transport::BELT_SURFACE)).abs() <= 0.08
        });
        let Some(i) = candidate else {
            continue;
        };
        let original = game.dropped[i].stack.clone();
        let Some(entity) = game.world.block_entities.get(p) else {
            continue;
        };
        let before = entity.slots().to_vec();
        if before.len() < 18 {
            continue;
        }
        let Some((left, into)) = machines::transfer(
            &portable(&[Some(original.clone())]),
            &portable(&before[machines::INPUT]),
            64,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            continue;
        };
        let mut originals = before.clone();
        originals.push(Some(original));
        let (Some(left), Some(after)) = (
            restore(&left.after, &originals),
            restore(&into.after, &originals),
        ) else {
            continue;
        };
        let Some(entity) = game.world.block_entities.get_mut(p) else {
            continue;
        };
        if entity.slots() != before {
            continue;
        }
        entity.slots_mut()[machines::INPUT].clone_from_slice(&after);
        if let Some(left) = left.into_iter().next().flatten() {
            game.dropped[i].stack = left;
        } else {
            game.dropped.remove(i);
        }
        dirty_machine(game, *p);
    }
}
pub(super) fn use_depot(game: &mut Game<'_>, p: [i32; 3]) -> Result<bool> {
    if game.remote.is_some() || !game.may_build() {
        return Ok(false);
    }
    let pos = (p[0], p[1], p[2]);
    if machine_name(game, pos) != "create:depot" {
        return Ok(false);
    }
    let held = game.held().cloned();
    if held.as_ref().is_some_and(|s| &*s.item == "create:wrench") {
        return Ok(false);
    }
    let sneak = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    if sneak {
        if held.is_some() {
            return Ok(false);
        }
        ensure_machine(game, pos);
        game.container = Some(pos);
        game.screen = Screen::Inventory;
        game.capture(false);
        game.right = false;
        return Ok(true);
    }
    ensure_machine(game, pos);
    let Some(entity) = game.world.block_entities.get(&pos) else {
        return Ok(false);
    };
    let before = entity.slots().to_vec();
    if before.len() < 18 {
        return Ok(false);
    }
    if let Some(held) = held {
        let Some((left, into)) = machines::transfer(
            &portable(&[Some(held.clone())]),
            &portable(&before[machines::INPUT]),
            64,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            return Ok(true);
        };
        let mut originals = before.clone();
        originals.push(Some(held.clone()));
        let (Some(left), Some(after)) = (
            restore(&left.after, &originals),
            restore(&into.after, &originals),
        ) else {
            return Ok(true);
        };
        if game.held() != Some(&held) {
            return Ok(true);
        }
        let Some(entity) = game.world.block_entities.get_mut(&pos) else {
            return Ok(true);
        };
        if entity.slots() != before {
            return Ok(true);
        }
        entity.slots_mut()[machines::INPUT].clone_from_slice(&after);
        if !game.creative {
            *game.held_mut() = left.into_iter().next().flatten();
        }
    } else {
        let slot = machines::OUTPUT
            .chain(machines::INPUT)
            .chain(18..before.len())
            .find(|&i| before[i].is_some());
        let Some(slot) = slot else {
            return Ok(true);
        };
        let stack = before[slot].clone().unwrap();
        let left = game.give(stack);
        if let Some(entity) = game.world.block_entities.get_mut(&pos) {
            entity.slots_mut()[slot] = left;
        }
    }
    dirty_machine(game, pos);
    send_depot_visuals(game, &[(pos, "create:depot".into())], true);
    Ok(true)
}

#[derive(Clone, Serialize, Deserialize)]
struct DepotVisual {
    pos: [i32; 3],
    item: Option<machines::Stack>,
}
#[derive(Default)]
struct DepotCache {
    context: Option<(std::path::PathBuf, String, usize)>,
    items: BTreeMap<MachinePos, Option<ItemStack>>,
}
thread_local! {static DEPOT_VISUALS:std::cell::RefCell<DepotCache>=Default::default();}
fn depot_display(game: &Game<'_>, p: MachinePos) -> Option<ItemStack> {
    let entity = game.world.block_entities.get(&p)?;
    let slots = entity.slots();
    machines::OUTPUT
        .chain(machines::INPUT)
        .filter_map(|i| slots.get(i).and_then(Option::as_ref))
        .next()
        .cloned()
}
fn send_depot_visuals(game: &Game<'_>, positions: &[(MachinePos, String)], force: bool) {
    let Some(lan) = &game.lan else {
        return;
    };
    thread_local! {static LAST_VISUAL:std::cell::RefCell<BTreeMap<(std::path::PathBuf,String,MachinePos),String>>=const{std::cell::RefCell::new(BTreeMap::new())};}
    let tick = game.world.game_time;
    let mut entries = Vec::new();
    for (p, name) in positions {
        if name != "create:depot" {
            continue;
        }
        let item = depot_display(game, *p);
        let item = portable(&[item]).into_iter().next().flatten();
        let Ok(encoded) = serde_json::to_string(&item) else {
            continue;
        };
        let changed = LAST_VISUAL.with(|old| {
            if old.borrow().len() > 8192 {
                old.borrow_mut().clear();
            }
            old.borrow_mut()
                .insert(
                    (game.save.root.clone(), format!("{:?}", game.dim), *p),
                    encoded.clone(),
                )
                .as_deref()
                != Some(&encoded)
        });
        if encoded.len() <= 65536 && (force || changed || tick.rem_euclid(5) == 0) {
            entries.push(DepotVisual {
                pos: [p.0, p.1, p.2],
                item,
            });
        }
    }
    for batch in entries.chunks(8) {
        if let Ok(data) = serde_json::to_vec(&(1u32, format!("{:?}", game.dim), batch)) {
            if data.len() <= 1_000_000 {
                lan.hub.broadcast_level(
                    &horizon_protocol::packets::play::Clientbound::CustomPayload(
                        horizon_protocol::packets::configuration::CustomPayload {
                            channel: "create:depot_visuals_v1".into(),
                            data,
                        },
                    ),
                );
            }
        }
    }
}
fn receive_depot_visuals(
    game: &mut Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) -> bool {
    let horizon_protocol::packets::play::Clientbound::CustomPayload(payload) = packet else {
        return false;
    };
    if payload.channel != "create:depot_visuals_v1" {
        return false;
    }
    if payload.data.len() > 1_000_000 {
        return true;
    }
    let Ok((version, dim, values)) =
        serde_json::from_slice::<(u32, String, Vec<DepotVisual>)>(&payload.data)
    else {
        return true;
    };
    if version != 1 || dim != format!("{:?}", game.dim) || values.len() > 32 {
        return true;
    }
    DEPOT_VISUALS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let context = client_context(game);
        if cache.context.as_ref() != Some(&context) {
            cache.context = Some(context);
            cache.items.clear();
        }
        cache.items.retain(|p, _| game.world.loaded(p.0, p.2));
        for v in values {
            let p = (v.pos[0], v.pos[1], v.pos[2]);
            if !game.world.loaded(p.0, p.2) {
                continue;
            }
            let item = match v.item {
                None => None,
                Some(s) => {
                    let Some(item) = decode_portable_item(&s) else {
                        continue;
                    };
                    Some(item)
                }
            };
            cache.items.insert(p, item);
        }
    });
    true
}
pub(super) fn draw_surface_world(
    game: &mut Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
) {
    let positions = all_create(game);
    for (p, name) in positions {
        if matches!(
            name.as_str(),
            "create:mechanical_press" | "create:mechanical_mixer"
        ) {
            draw_moving_machine(game, p, &name, camera, alpha, out);
        }
        if name != "create:depot" {
            continue;
        }
        let item = DEPOT_VISUALS
            .with(|cache| {
                let cache = cache.borrow();
                (cache.context.as_ref() == Some(&client_context(game)))
                    .then(|| cache.items.get(&p).cloned())
                    .flatten()
            })
            .unwrap_or_else(|| depot_display(game, p));
        let Some(item) = item else {
            continue;
        };
        let at = glam::DVec3::new(
            p.0 as f64 + 0.5,
            p.1 as f64 + transport::BELT_SURFACE + 0.02,
            p.2 as f64 + 0.5,
        ) - camera;
        if at.length_squared() > 64.0 * 64.0 {
            continue;
        }
        let model = game.resolve_item(
            &item,
            horizon_assets::item_model::DisplayContext::Ground,
            None,
            0,
        );
        if model.is_empty() {
            continue;
        }
        let (lo, _) = model.bounding_box();
        let pose = glam::Mat4::from_translation(at.as_vec3() - glam::Vec3::new(0.0, lo.y, 0.0))
            * glam::Mat4::from_rotation_y(std::f32::consts::FRAC_PI_4);
        game.submit_item_world(
            &model,
            pose,
            horizon_client::block_entity_render::vertex_light(game.world.light(p.0, p.1, p.2)),
            out,
        );
    }
}

fn work_phase(game: &Game<'_>, p: MachinePos) -> f32 {
    let mut state = read_machine(game, p);
    let extra = CLIENT_MACHINES.with(|cache| {
        let cache = cache.borrow();
        if cache.context.as_ref() != Some(&client_context(game)) {
            return 0.0;
        }
        cache
            .values
            .get(&p)
            .map_or(0.0, |v| v.received.elapsed().as_secs_f32().min(0.25) * 20.0)
    });
    if state.status == "Working" {
        state.progress += extra * (state.rpm.abs() / 8.0).clamp(0.125, 32.0);
    }
    state
        .recipe
        .as_deref()
        .and_then(|id| recipes::active_recipes().iter().find(|r| r.id == id))
        .map_or(0.0, |r| {
            (state.progress / r.work.max(1) as f32).clamp(0.0, 1.0)
        })
}
fn draw_part(
    game: &mut Game<'_>,
    item: &str,
    part: &str,
    pose: glam::Mat4,
    light: u32,
    out: &mut Vec<Vertex>,
) {
    let mut stack = ItemStack::new(item, 1);
    stack.data_mut().set_other(
        "minecraft:item_model",
        Some(Tag::String(
            format!("create:horizon_{part}").as_str().into(),
        )),
    );
    let model = game.resolve_item(
        &stack,
        horizon_assets::item_model::DisplayContext::None,
        None,
        0,
    );
    game.submit_item_world(&model, pose, light, out);
}
fn draw_moving_machine(
    game: &mut Game<'_>,
    p: MachinePos,
    name: &str,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
) {
    let at = glam::DVec3::new(p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5) - camera;
    if at.length_squared() > 64.0 * 64.0 {
        return;
    }
    let phase = work_phase(game, p);
    let angle = rotation_angle(game, p, alpha as f32).to_radians();
    let light = horizon_client::block_entity_render::vertex_light(game.world.light(p.0, p.1, p.2));
    let mut clearance = if name == "create:mechanical_press" {
        0.9375
    } else {
        0.8125
    };
    for dy in [1, 2] {
        if matches!(
            machine_name(game, (p.0, p.1 - dy, p.2)).as_str(),
            "create:belt" | "create:depot" | "create:basin"
        ) {
            clearance = if name == "create:mechanical_press" {
                (dy as f32 - 0.25 - transport::BELT_SURFACE as f32).clamp(0.0, 1.0)
            } else {
                (dy as f32 - 0.6875 - 0.5).clamp(0.0, 1.0)
            };
            break;
        }
    }
    let base = glam::Mat4::from_translation(at.as_vec3());
    if name == "create:mechanical_press" {
        let yaw = match facing_step(game, p) {
            [1, 0, 0] => std::f32::consts::FRAC_PI_2,
            [-1, 0, 0] => -std::f32::consts::FRAC_PI_2,
            [0, 0, 1] => std::f32::consts::PI,
            _ => 0.0,
        };
        let base = base * glam::Mat4::from_rotation_y(-yaw);
        draw_part(
            game,
            name,
            "press_shaft",
            base * glam::Mat4::from_rotation_z(angle),
            light,
            out,
        );
        draw_part(
            game,
            name,
            "press_head",
            base * glam::Mat4::from_translation(glam::Vec3::new(
                0.0,
                -surface::press_stroke(phase, clearance),
                0.0,
            )),
            light,
            out,
        );
    } else {
        let lowered = base
            * glam::Mat4::from_translation(glam::Vec3::new(
                0.0,
                -surface::mixer_stroke(phase, clearance),
                0.0,
            ));
        draw_part(
            game,
            name,
            "mixer_gear",
            base * glam::Mat4::from_rotation_y(angle),
            light,
            out,
        );
        draw_part(game, name, "mixer_pole", lowered, light, out);
        draw_part(
            game,
            name,
            "mixer_head",
            lowered * glam::Mat4::from_rotation_y(angle * 4.0),
            light,
            out,
        );
    }
}

fn tick_crushing_pairs(
    game: &mut Game<'_>,
    positions: &[(MachinePos, String)],
    speeds: &std::collections::HashMap<[i32; 3], f32>,
    run: &mut SurfaceRun,
) {
    let wheels: BTreeSet<_> = positions
        .iter()
        .filter(|(_, name)| name == "create:crushing_wheel")
        .map(|(p, _)| *p)
        .collect();
    for p in &wheels {
        let Some(state) = game
            .pack
            .assets
            .registry
            .state(game.world.block(p.0, p.1, p.2))
        else {
            continue;
        };
        let axis = match read_axis(state) {
            Axis::X => transmission::Axis::X,
            Axis::Z => transmission::Axis::Z,
            _ => {
                clear_world_work(game, *p);
                continue;
            }
        };
        let q = if axis == transmission::Axis::X {
            (p.0, p.1, p.2 + 2)
        } else {
            (p.0 + 2, p.1, p.2)
        };
        // The other wheel can belong to an earlier pair; don't clear its mirrored status.
        if run.handled.contains(p) {
            continue;
        }
        if !wheels.contains(&q) {
            clear_world_work(game, *p);
            continue;
        }
        let same_axis = game
            .pack
            .assets
            .registry
            .state(game.world.block(q.0, q.1, q.2))
            .is_some_and(|s| read_axis(s) == read_axis(state));
        let a = speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0);
        let b = speeds.get(&[q.0, q.1, q.2]).copied().unwrap_or(0.0);
        let Some((center, rpm)) =
            surface::crushing_pair([p.0, p.1, p.2], [q.0, q.1, q.2], axis, a, b)
                .filter(|_| same_axis)
        else {
            clear_world_work(game, *p);
            continue;
        };
        if has_machine_inputs(game, *p) || has_machine_inputs(game, q) {
            clear_world_work(game, *p);
            continue;
        }
        let catalog = surface_catalog(game, "crushing");
        let state = read_machine(game, *p);
        let mut candidates: Vec<_> = game
            .dropped
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.target.is_none()
                    && !run.held.contains(&item.id)
                    && surface::in_crushing_gap(
                        center,
                        [item.position.x, item.position.y, item.position.z],
                    )
                    && {
                        let input = portable(&[Some(item.stack.clone())]);
                        surface::recipe(&state, input[0].as_ref().unwrap(), "crushing", &catalog)
                            .is_some()
                    }
            })
            .map(|(i, item)| (i, item.id))
            .collect();
        candidates
            .sort_by_key(|(_, id)| (!state.world_target.is_some_and(|b| b.entity == *id), *id));
        let Some((index, id)) = candidates.first().copied() else {
            clear_world_work(game, *p);
            continue;
        };
        let before = game.dropped[index].stack.clone();
        let input = portable(&[Some(before.clone())]);
        let Ok(update) = surface::work(
            &state,
            id,
            input[0].as_ref().unwrap(),
            "crushing",
            rpm,
            machines::Heat::None,
            &catalog,
            |id| game.items().get(id).map_or(1, |i| i.max_stack),
        ) else {
            continue;
        };
        if !commit_surface_stack(game, index, id, &before, &update) {
            continue;
        }
        if update.remaining.is_some() {
            run.held.insert(id);
            if let Some(item) = game.dropped.iter_mut().find(|e| e.id == id) {
                item.position.x = center[0];
                item.position.y = center[1];
                item.position.z = center[2];
                item.velocity.x = 0.0;
                item.velocity.y = 0.04;
                item.velocity.z = 0.0;
            }
        }
        let mut partner = read_machine(game, q);
        partner.progress = update.state.progress;
        partner.recipe = update.state.recipe.clone();
        partner.status = update.state.status.clone();
        store_machine(game, q, partner);
        store_machine(game, *p, update.state);
        run.handled.insert(*p);
        run.handled.insert(q);
    }
}

// Top processors own no extra item inventory: input remains a host dropped stack.
fn tick_top_processors(
    game: &mut Game<'_>,
    positions: &[(MachinePos, String)],
    speeds: &std::collections::HashMap<[i32; 3], f32>,
    run: &mut SurfaceRun,
) {
    for (p, name) in positions {
        if name=="create:deployer" && tick_deployer_placement(game,*p,speeds.get(&[p.0,p.1,p.2]).copied().unwrap_or(0.0)) {
            run.handled.insert(*p);
            continue;
        }
        if name == "create:mechanical_saw" && facing_step(game,*p)[1]==0 && !has_machine_inputs(game,*p) {
            tick_tree_saw(game,*p,speeds.get(&[p.0,p.1,p.2]).copied().unwrap_or(0.0));
            run.handled.insert(*p);
            continue;
        }
        let (station, height, rpm) = match name.as_str() {
            "create:item_drain" => ("emptying", 0.8125, 32.0),
            "create:mechanical_saw" if facing_step(game, *p) == [0, 1, 0] => {
                ("cutting", 0.75, speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0))
            }
            _ => continue,
        };
        if has_machine_inputs(game, *p) {
            clear_world_work(game, *p);
            continue;
        }
        let state = read_machine(game, *p);
        let catalog: Vec<_> = surface_catalog(game, station).into_iter().filter(|r| station != "cutting" || recipes::output_matches(r,state.item_filter.as_deref())).collect();
        let mut candidates: Vec<_> = game.dropped.iter().enumerate()
            .filter(|(_, item)| {
                item.target.is_none() && !run.held.contains(&item.id)
                    && surface::on_processor_top([p.0,p.1,p.2], height,
                        [item.position.x,item.position.y,item.position.z])
                    && {
                        let input = portable(&[Some(item.stack.clone())]);
                        surface::recipe(&state,input[0].as_ref().unwrap(),station,&catalog).is_some()
                    }
            })
            .map(|(i,item)| (i,item.id)).collect();
        candidates.sort_by_key(|(_,id)| (!state.world_target.is_some_and(|b| b.entity == *id),*id));
        let Some((index,id)) = candidates.first().copied() else {
            clear_world_work(game,*p);
            continue;
        };
        let before = game.dropped[index].stack.clone();
        let input = portable(&[Some(before.clone())]);
        let Ok(mut update) = surface::work(&state,id,input[0].as_ref().unwrap(),station,rpm,
            machines::Heat::None,&catalog,|id| game.items().get(id).map_or(1,|i| i.max_stack)) else {continue};
        if update.result == machines::TickResult::NoPower
            && matches!(state.status.as_str(), "Overstressed" | "Conflict") {
            update.state.status = state.status.clone();
        }
        if !commit_surface_stack(game,index,id,&before,&update) {continue;}
        if rpm != 0.0 && update.remaining.as_ref().is_some_and(|s|
            surface::recipe(&update.state,s,station,&catalog).is_some()) {
            run.held.insert(id);
            if let Some(item) = game.dropped.iter_mut().find(|e| e.id == id) {
                item.position.x = p.0 as f64 + 0.5;
                item.position.z = p.2 as f64 + 0.5;
                item.velocity.x = 0.0;
                item.velocity.z = 0.0;
            }
        }
        store_machine(game,*p,update.state);
        run.handled.insert(*p);
    }
}

pub(super) fn use_saw_filter(game: &mut Game<'_>, p: [i32;3]) -> Result<bool> {
    if game.remote.is_some() || !game.may_build() {return Ok(false);}
    let pos=(p[0],p[1],p[2]);
    if machine_name(game,pos)!="create:mechanical_saw" {return Ok(false);}
    let held=game.held().cloned();
    let sneaking=game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let mut state=read_machine(game,pos);
    if let Some(held)=held {
        if sneaking || &*held.item=="create:wrench" || !recipes::active_recipes().iter()
            .any(|r|r.station=="cutting" && recipes::output_matches(r,Some(&held.item))) {return Ok(false);}
        state.item_filter=Some(held.item.to_string());
    } else if sneaking {
        state.item_filter=None;
    } else {return Ok(false);}
    state.progress=0.0;state.recipe=None;state.world_target=None;
    let text=format!("Saw output filter: {}",state.item_filter.as_deref().unwrap_or("automatic"));
    store_machine(game,pos,state);transport_notice(game,pos,&text);Ok(true)
}

fn tick_tree_saw(game: &mut Game<'_>, p: MachinePos, rpm: f32) {
    let step=facing_step(game,p);
    if step[1]!=0 || has_machine_inputs(game,p) {return;}
    let mut state=read_machine(game,p);
    if rpm==0.0 {return;}
    let target=offset_machine(p,step,1);
    let id=game.world.block(target.0,target.1,target.2);
    if !game.world.loaded(target.0,target.2) || machines::trees::family(&game.res.facts().get(id).name).is_none() {
        if state.recipe.as_deref()==Some("create:world/tree_felling") {state.progress=0.0;state.recipe=None;store_machine(game,p,state);}
        return;
    }
    if state.recipe.as_deref()!=Some("create:world/tree_felling") {state.progress=0.0;}
    state.recipe=Some("create:world/tree_felling".into());
    state.progress=(state.progress+(rpm.abs()/32.0).clamp(0.03125,8.0)).min(20.0);
    state.status="SawingTree".into();
    if state.progress>=20.0 {
        match machines::trees::plan([target.0,target.1,target.2],|q| {
            game.world.loaded(q[0],q[2]).then(||game.res.facts().get(game.world.block(q[0],q[1],q[2])).name.to_string())
        }) {
            Ok(logs)=>{
                let mut count=0;
                for q in logs {
                    if game.destroy_block_dropping(q).is_err() {break;}
                    count+=1;
                }
                state.progress=0.0;state.recipe=None;state.status=format!("Felled {count} logs");
            }
            Err(reason)=>{state.progress=0.0;state.status=format!("Tree {reason:?}");}
        }
    }
    store_machine(game,p,state);
}

// Authored XP-nugget behavior; remote use remains a server integration gap.
pub(super) fn use_create_item(game: &mut Game<'_>) -> bool {
    if game.remote.is_some() || !game.may_build() {return false;}
    if !game.held().is_some_and(|s| &*s.item=="create:experience_nugget" && s.count>0) {return false;}
    game.consume_held();
    game.experience.give(3);
    true
}
