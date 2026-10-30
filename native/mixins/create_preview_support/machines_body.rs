// Authoritative machine state lives with each chunk snapshot, alongside real inventories.
use horizon_world::nbt::Tag;
use serde::{Deserialize, Serialize};
type MachinePos = (i32, i32, i32);
const MACHINE_TAG: &str = "create:machines";
#[derive(Clone, Serialize, Deserialize)]
struct MachineRecord {
    pos: [i32; 3],
    name: String,
    data: machines::MachineState,
}
struct MachineBatch {
    world: usize,
    chunks: BTreeMap<(i32, i32), BTreeMap<MachinePos, MachineRecord>>,
    dirty: std::collections::BTreeSet<(i32, i32)>,
}
thread_local! {static BATCH:std::cell::RefCell<Option<MachineBatch>>=const{std::cell::RefCell::new(None)};}
// Network snapshots are cached, so drawing a shaft does not parse chunk JSON
// every frame. Each snapshot carries its server tick; local elapsed time drives
// the visual phase even when SetTime adjusts the client's world clock.
struct ClientSample {
    name: String,
    state: machines::MachineState,
    rotation: machines::RotationSample,
    received: std::time::Instant,
}
#[derive(Default)]
struct ClientMachines {
    context: Option<(std::path::PathBuf, String, usize)>,
    values: BTreeMap<MachinePos, ClientSample>,
}
thread_local! {
    static CLIENT_MACHINES: std::cell::RefCell<ClientMachines> = Default::default();
    static MENU_TARGET: Cell<Option<MachinePos>> = const { Cell::new(None) };
}
fn client_context(game: &Game<'_>) -> (std::path::PathBuf, String, usize) {
    (
        game.save.root.clone(),
        format!("{:?}", game.dim),
        &game.world as *const _ as usize,
    )
}
fn cached_machine(game: &Game<'_>, p: MachinePos) -> Option<machines::MachineState> {
    if !game.native_window() && game.remote.is_none() {
        return None;
    }
    CLIENT_MACHINES.with(|cache| {
        let cache = cache.borrow();
        if cache.context.as_ref() != Some(&client_context(game)) {
            return None;
        }
        let v = cache.values.get(&p)?;
        (v.name == machine_name(game, p)).then(|| v.state.clone())
    })
}
fn rotation_angle(game: &Game<'_>, p: MachinePos, alpha: f32) -> f32 {
    if let Some(angle) = CLIENT_MACHINES.with(|cache| {
        let cache = cache.borrow();
        if cache.context.as_ref() != Some(&client_context(game)) {
            return None;
        }
        let v = cache.values.get(&p)?;
        (v.name == machine_name(game, p))
            .then(|| v.rotation.angle_after(v.received.elapsed().as_secs_f64()))
    }) {
        return angle;
    }
    read_machine(game, p).render_angle(game.world.game_time.max(0) as u64, alpha)
}
pub(super) fn remember_menu_target(game: &Game<'_>, target: Option<[i32; 3]>) {
    MENU_TARGET.with(|t| {
        t.set(
            target
                .map(|p| (p[0], p[1], p[2]))
                .filter(|p| machine_name(game, *p).starts_with("create:")),
        )
    });
}
fn begin_machine_batch(game: &Game<'_>) {
    let mut chunks = BTreeMap::new();
    for &key in game.world.chunks.keys() {
        let values = records(game, key)
            .into_iter()
            .map(|r| ((r.pos[0], r.pos[1], r.pos[2]), r))
            .collect();
        chunks.insert(key, values);
    }
    BATCH.with(|b| {
        *b.borrow_mut() = Some(MachineBatch {
            world: &game.world as *const _ as usize,
            chunks,
            dirty: Default::default(),
        })
    });
}
fn finish_machine_batch(game: &mut Game<'_>) {
    if let Some(batch) = BATCH.with(|b| b.borrow_mut().take()) {
        for key in batch.dirty {
            if let Some(values) = batch.chunks.get(&key) {
                write_records(game, key, &values.values().cloned().collect::<Vec<_>>());
            }
        }
    }
}
fn machine_name(game: &Game<'_>, p: MachinePos) -> String {
    game.pack
        .assets
        .registry
        .state(game.world.block(p.0, p.1, p.2))
        .map_or(String::new(), |s| s.name.clone())
}
fn records(game: &Game<'_>, key: (i32, i32)) -> Vec<MachineRecord> {
    if let Some(values) = BATCH.with(|b| {
        b.borrow()
            .as_ref()
            .filter(|b| b.world == &game.world as *const _ as usize)
            .and_then(|b| b.chunks.get(&key))
            .map(|v| v.values().cloned().collect())
    }) {
        return values;
    }
    game.world
        .chunks
        .get(&key)
        .and_then(|c| c.saved.as_ref())
        .and_then(|s| s.root.get(&MACHINE_TAG.into()))
        .and_then(|t| {
            if let Tag::String(s) = t {
                serde_json::from_str::<Vec<MachineRecord>>(&s.to_string_lossy()).ok()
            } else {
                None
            }
        })
        .unwrap_or_default()
}
fn read_machine(game: &Game<'_>, p: MachinePos) -> machines::MachineState {
    if let Some(state) = cached_machine(game, p) {
        return state;
    }
    let name = machine_name(game, p);
    if let Some(data) = BATCH.with(|b| {
        b.borrow()
            .as_ref()
            .filter(|b| b.world == &game.world as *const _ as usize)
            .and_then(|b| b.chunks.get(&(p.0 >> 4, p.2 >> 4)))
            .and_then(|v| v.get(&p))
            .filter(|r| r.name == name)
            .map(|r| r.data.clone())
    }) {
        return data;
    }
    records(game, (p.0 >> 4, p.2 >> 4))
        .into_iter()
        .find(|r| r.pos == [p.0, p.1, p.2] && r.name == name)
        .and_then(|r| machines::MachineState::decode(&r.data.encode()).ok())
        .unwrap_or_default()
}
fn write_records(game: &mut Game<'_>, key: (i32, i32), values: &[MachineRecord]) {
    let deferred = BATCH.with(|b| {
        let mut b = b.borrow_mut();
        if let Some(b) = b
            .as_mut()
            .filter(|b| b.world == &game.world as *const _ as usize)
        {
            b.chunks.insert(
                key,
                values
                    .iter()
                    .cloned()
                    .map(|r| ((r.pos[0], r.pos[1], r.pos[2]), r))
                    .collect(),
            );
            b.dirty.insert(key);
            true
        } else {
            false
        }
    });
    if deferred {
        return;
    }
    let Ok(json) = serde_json::to_string(values) else {
        return;
    };
    let Some(chunk) = game.world.chunks.get_mut(&key) else {
        return;
    };
    let chunk = std::sync::Arc::make_mut(chunk);
    let saved = std::sync::Arc::make_mut(chunk.saved.get_or_insert_default());
    let tag = Tag::String(json.as_str().into());
    if saved.root.get(&MACHINE_TAG.into()) != Some(&tag) {
        saved.root.insert(MACHINE_TAG.into(), tag);
        game.world.modified.insert(key);
    }
}
fn store_machine(game: &mut Game<'_>, p: MachinePos, data: machines::MachineState) {
    let key = (p.0 >> 4, p.2 >> 4);
    let deferred = BATCH.with(|b| {
        let mut b = b.borrow_mut();
        if let Some(b) = b
            .as_mut()
            .filter(|b| b.world == &game.world as *const _ as usize)
        {
            b.chunks.entry(key).or_default().insert(
                p,
                MachineRecord {
                    pos: [p.0, p.1, p.2],
                    name: machine_name(game, p),
                    data: data.clone(),
                },
            );
            b.dirty.insert(key);
            true
        } else {
            false
        }
    });
    if deferred {
        return;
    }
    let mut saved = records(game, key);
    saved.retain(|r| r.pos != [p.0, p.1, p.2]);
    saved.push(MachineRecord {
        pos: [p.0, p.1, p.2],
        name: machine_name(game, p),
        data,
    });
    write_records(game, key, &saved);
}
fn dirty_machine(game: &mut Game<'_>, p: MachinePos) {
    game.container_changed(p);
    game.mark_block_entity_updated(p);
}
fn portable(slots: &[Option<ItemStack>]) -> Vec<Option<machines::Stack>> {
    slots
        .iter()
        .map(|s| {
            s.as_ref().map(|s| machines::Stack {
                id: s.item.to_string(),
                count: s.count,
                components: horizon_client::item_data::encode_components(s)
                    .map(|t| horizon_client::snbt::to_snbt(&t))
                    .unwrap_or_default(),
            })
        })
        .collect()
}
fn restore(
    slots: &[Option<machines::Stack>],
    originals: &[Option<ItemStack>],
) -> Option<Vec<Option<ItemStack>>> {
    let known = portable(originals);
    slots
        .iter()
        .map(|s| match s {
            None => Some(None),
            Some(s) => {
                if let Some(index) = known.iter().position(|o| {
                    o.as_ref()
                        .is_some_and(|o| o.id == s.id && o.components == s.components)
                }) {
                    let mut item = originals[index].clone()?;
                    item.count = s.count;
                    Some(Some(item))
                } else if s.components.is_empty() {
                    Some(Some(ItemStack::new(&s.id, s.count)))
                } else {
                    None
                }
            }
        })
        .collect()
}
fn ensure_machine(game: &mut Game<'_>, p: MachinePos) {
    let name = machine_name(game, p);
    let Some(size) = machines::container_size(&name) else {
        return;
    };
    game.world.block_entities.get_or_insert_with(p, || {
        horizon_client::containers::BlockEntity::Chest(vec![None; size])
    });
    if let Some(horizon_client::containers::BlockEntity::Chest(items)) =
        game.world.block_entities.get_mut(&p)
    {
        if items.len() < size {
            items.resize(size, None)
        } else if items[size..].iter().all(Option::is_none) {
            items.truncate(size)
        }
    }
    if size >= 18 {
        game.world.block_names.entry(p).or_insert_with(|| {
            name.trim_start_matches("create:")
                .split('_')
                .map(|s| {
                    let mut c = s.chars();
                    c.next().map_or(String::new(), |n| {
                        n.to_uppercase().collect::<String>() + c.as_str()
                    })
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
    }
}
#[derive(Default)]
struct MachineScan {
    sections: BTreeMap<
        (i32, usize, i32),
        (
            std::sync::Arc<horizon_client::chunk::Section>,
            Vec<(MachinePos, String)>,
        ),
    >,
}
fn all_create(game: &Game<'_>) -> Vec<(MachinePos, String)> {
    thread_local! {static SCANS:std::cell::RefCell<BTreeMap<std::path::PathBuf,MachineScan>>=const{std::cell::RefCell::new(BTreeMap::new())};}
    SCANS.with(|all| {
        let mut all = all.borrow_mut();
        if all.len() > 8 {
            all.clear()
        }
        let cache = all.entry(game.save.root.clone()).or_default();
        let mut live = std::collections::BTreeSet::new();
        let mut out = vec![];
        for (&(cx, cz), chunk) in &game.world.chunks {
            for (sy, section) in chunk.sections.iter().enumerate() {
                let key = (cx, sy, cz);
                live.insert(key);
                if cache
                    .sections
                    .get(&key)
                    .is_none_or(|(old, _)| !std::sync::Arc::ptr_eq(old, section))
                {
                    let mut found = vec![];
                    if !section.blocks.uniform().is_some_and(|id| {
                        game.pack
                            .assets
                            .registry
                            .state(BlockStateId(id))
                            .is_none_or(|s| !s.name.starts_with("create:"))
                    }) {
                        for i in 0..4096 {
                            if let Some(s) = game
                                .pack
                                .assets
                                .registry
                                .state(BlockStateId(section.get(i)))
                            {
                                if machines::container_size(&s.name).is_some() {
                                    found.push((
                                        (
                                            cx * 16 + (i & 15) as i32,
                                            chunk.min_y() + sy as i32 * 16 + (i >> 8) as i32,
                                            cz * 16 + ((i >> 4) & 15) as i32,
                                        ),
                                        s.name.clone(),
                                    ));
                                }
                            }
                        }
                    }
                    cache
                        .sections
                        .insert(key, (std::sync::Arc::clone(section), found));
                }
                out.extend(cache.sections[&key].1.iter().cloned());
            }
        }
        cache.sections.retain(|k, _| live.contains(k));
        out.sort_by_key(|x| x.0);
        out
    })
}
fn process_with_fluid_owner(
    game: &mut Game<'_>,
    p: MachinePos,
    owner: MachinePos,
    station: &str,
    rpm: f32,
    heat: machines::Heat,
) -> Option<machines::TickResult> {
    ensure_machine(game, p);
    let before = match game.world.block_entities.get(&p) {
        Some(e) => e.slots().to_vec(),
        None => return None,
    };
    if before.len() < 18 {
        return None;
    }
    let mut state = read_machine(game, owner);
    state.set_speed(rpm, game.world.game_time.max(0) as u64);
    let input = portable(&before);
    let catalog: Vec<_> = recipes::catalog::recipes()
        .iter()
        .filter(|r| {
            (r.station == station || (station == "compacting" && r.station == "pressing"))
                && r.products
                    .iter()
                    .all(|p| p.fluid || game.res.items().get(p.id).is_some())
        })
        .copied()
        .collect();
    let Ok(mut update) =
        machines::process_with_catalog(&state, &input, station, rpm, heat, &catalog, |id| {
            game.res.items().get(id).map_or(1, |i| i.max_stack)
        })
    else {
        return None;
    };
    if update.result == machines::TickResult::NoPower
        && matches!(state.status.as_str(), "Overstressed" | "Conflict")
    {
        update.state.status = state.status.clone();
    }
    if update
        .inventory
        .after
        .iter()
        .flatten()
        .any(|s| game.res.items().get(&s.id).is_none())
    {
        return None;
    }
    let Some(after) = restore(&update.inventory.after, &before) else {
        return None;
    };
    if let Some(entity) = game.world.block_entities.get_mut(&p) {
        if portable(entity.slots()) != update.inventory.before {
            return None;
        }
        let changed = entity.slots() != after;
        entity.slots_mut().clone_from_slice(&after);
        if changed {
            dirty_machine(game, p);
        }
    }
    let result = update.result;
    store_machine(game, owner, update.state);
    Some(result)
}
fn heat_at(game: &Game<'_>, p: MachinePos, tick: u64) -> machines::Heat {
    for dy in [1, 2, 3] {
        let below = (p.0, p.1 - dy, p.2);
        if machine_name(game, below) == "create:blaze_burner" {
            let s = read_machine(game, below);
            return if s.superheat_until > tick {
                machines::Heat::Superheated
            } else if s.burn_until > tick {
                machines::Heat::Heated
            } else {
                machines::Heat::None
            };
        }
    }
    machines::Heat::None
}
fn move_item(game: &mut Game<'_>, from: MachinePos, to: MachinePos) {
    if from == to {
        return;
    }
    let a = match game.world.block_entities.get(&from) {
        Some(e) => e.slots().to_vec(),
        None => return,
    };
    let b = match game.world.block_entities.get(&to) {
        Some(e) => e.slots().to_vec(),
        None => return,
    };
    let ar = if machines::station(&machine_name(game, from)).is_some() {
        9..18.min(a.len())
    } else {
        0..a.len()
    };
    let br = if machines::station(&machine_name(game, to)).is_some() {
        0..9.min(b.len())
    } else {
        0..b.len()
    };
    if ar.is_empty() || br.is_empty() {
        return;
    }
    let aa = portable(&a[ar.clone()]);
    let bb = portable(&b[br.clone()]);
    let Some((ap, bp)) = machines::transfer(&aa, &bb, 1, |id| {
        game.res.items().get(id).map_or(1, |i| i.max_stack)
    }) else {
        return;
    };
    let mut originals = a.clone();
    originals.extend(b.clone());
    let (Some(av), Some(bv)) = (
        restore(&ap.after, &originals),
        restore(&bp.after, &originals),
    ) else {
        return;
    };
    if game
        .world
        .block_entities
        .get(&from)
        .is_none_or(|e| e.slots() != a)
        || game
            .world
            .block_entities
            .get(&to)
            .is_none_or(|e| e.slots() != b)
    {
        return;
    }
    if let Some(e) = game.world.block_entities.get_mut(&from) {
        e.slots_mut()[ar].clone_from_slice(&av)
    }
    if let Some(e) = game.world.block_entities.get_mut(&to) {
        e.slots_mut()[br].clone_from_slice(&bv)
    }
    dirty_machine(game, from);
    dirty_machine(game, to);
}
fn pump_fluids(game: &mut Game<'_>, p: MachinePos, axis: transmission::Axis, rpm: f32) {
    if rpm == 0.0 {
        return;
    }
    let mut step = [0; 3];
    step[axis.index()] = if rpm > 0.0 { 1 } else { -1 };
    let endpoints = |game: &Game<'_>, sign: i32| {
        let mut out = vec![];
        for n in 1..=16 {
            let q = (
                p.0 + step[0] * n * sign,
                p.1 + step[1] * n * sign,
                p.2 + step[2] * n * sign,
            );
            let name = machine_name(game, q);
            if name == "create:fluid_pipe" {
                continue;
            }
            if machines::container_size(&name).is_some() {
                out.push(q)
            }
            break;
        }
        out
    };
    let a = endpoints(game, -1);
    let b = endpoints(game, 1);
    let (Some(&from), Some(&to)) = (a.first(), b.first()) else {
        return;
    };
    let sa = read_machine(game, from);
    let sb = read_machine(game, to);
    if let Some((sa, sb, _)) =
        machines::transfer_fluid(&sa, &sb, (rpm.abs() * 4.0).round().max(1.0) as u32)
    {
        store_machine(game, from, sa);
        store_machine(game, to, sb);
    }
}
fn broadcast_machine_state(game: &Game<'_>, values: &[MachineRecord], notice: &str) {
    if let Some(lan) = &game.lan {
        for batch in values.chunks(128) {
            if let Ok(data) = serde_json::to_vec(&(
                2u32,
                format!("{:?}", game.dim),
                game.world.game_time.max(0) as u64,
                batch,
                notice,
            )) {
                if data.len() < 1_000_000 {
                    lan.hub.broadcast_level(
                        &horizon_protocol::packets::play::Clientbound::CustomPayload(
                            horizon_protocol::packets::configuration::CustomPayload {
                                channel: "create:machines_v2".into(),
                                data,
                            },
                        ),
                    );
                }
            }
        }
    }
}
pub(super) fn receive_machines(
    game: &mut Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) -> bool {
    let horizon_protocol::packets::play::Clientbound::CustomPayload(payload) = packet else {
        return false;
    };
    if payload.channel != "create:machines_v2" {
        return false;
    }
    if payload.data.len() > 1_000_000 {
        return true;
    }
    if let Ok((version, dim, server_tick, values, notice)) =
        serde_json::from_slice::<(u32, String, u64, Vec<MachineRecord>, String)>(&payload.data)
    {
        if version != 2 || dim != format!("{:?}", game.dim) || values.len() > 128 {
            return true;
        }
        CLIENT_MACHINES.with(|cache| {
            let mut cache = cache.borrow_mut();
            let context = client_context(game);
            if cache.context.as_ref() != Some(&context) {
                cache.context = Some(context);
                cache.values.clear();
            }
            cache.values.retain(|p, _| game.world.loaded(p.0, p.2));
            for r in values {
                if !r.name.starts_with("create:")
                    || machines::MachineState::decode(&r.data.encode()).is_err()
                {
                    continue;
                }
                let p = (r.pos[0], r.pos[1], r.pos[2]);
                if machine_name(game, p) == r.name {
                    let rotation = machines::RotationSample::capture(&r.data, server_tick);
                    cache.values.insert(
                        p,
                        ClientSample {
                            name: r.name,
                            state: r.data,
                            rotation,
                            received: std::time::Instant::now(),
                        },
                    );
                }
            }
        });
        if !notice.is_empty() && notice.len() < 512 {
            game.native_chat(&notice);
        }
    }
    true
}
pub(super) fn tick_machines(game: &mut Game<'_>) {
    if game.native_window() || game.remote.is_some() || game.title.is_some() {
        return;
    }
    thread_local! {static LAST:std::cell::RefCell<BTreeMap<(std::path::PathBuf,String),i64>>=const{std::cell::RefCell::new(BTreeMap::new())};}
    let tick = game.world.game_time.max(0) as u64;
    let duplicate = LAST.with(|m| {
        m.borrow_mut().insert(
            (game.save.root.clone(), format!("{:?}", game.dim)),
            tick as i64,
        ) == Some(tick as i64)
    });
    if duplicate {
        return;
    }
    generate_zinc(game);
    begin_machine_batch(game);
    let positions = all_create(game);
    for &(p, _) in &positions {
        ensure_machine(game, p);
    }
    let mut nodes = BTreeMap::new();
    for (p, name) in &positions {
        let Some(state) = game
            .pack
            .assets
            .registry
            .state(game.world.block(p.0, p.1, p.2))
        else {
            continue;
        };
        let props = state
            .properties
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        let Some(mut node) = transmission::state::parse_state(
            &transmission::state::Registry::new(),
            &format!("{name}[{props}]"),
        ) else {
            continue;
        };
        let saved = read_machine(game, *p);
        node.direction = if node.kind == transmission::Kind::Belt {
            node.direction * saved.direction
        } else {
            saved.direction
        };
        match node.kind {
            transmission::Kind::CreativeMotor => node.configured_rpm = Some(saved.motor_rpm),
            transmission::Kind::HandCrank => node.active = saved.crank_until > tick,
            transmission::Kind::WaterWheel | transmission::Kind::LargeWaterWheel => {
                let a = match node.axis {
                    transmission::Axis::X => Axis::X,
                    transmission::Axis::Y => Axis::Y,
                    transmission::Axis::Z => Axis::Z,
                };
                let mut network = Network::default();
                network
                    .nodes
                    .insert(Pos(p.0, p.1, p.2), Node::new(a, Kind::WaterWheel));
                let flow = flows(game, &network);
                let rpm = kinetic::wheel_rpm(
                    a,
                    flow.get(&Pos(p.0, p.1, p.2))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                );
                node.active = rpm != 0.0;
                node.direction = if rpm < 0.0 { -1 } else { 1 };
            }
            transmission::Kind::Clutch | transmission::Kind::Gearshift => {
                let power = game.with_level(|b, l, t| {
                    b.has_neighbor_signal(l, t, BlockPos::new(p.0, p.1, p.2))
                });
                if node.kind == transmission::Kind::Clutch {
                    node.active = saved.engaged && !power
                } else if power {
                    node.direction = -node.direction;
                }
            }
            _ => {}
        }
        nodes.insert([p.0, p.1, p.2], node);
    }
    let solved = transmission::solve(&nodes);
    for (p, _) in &positions {
        let mut s = read_machine(game, *p);
        s.set_speed(
            solved.speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0),
            tick,
        );
        if let Some(n) = solved
            .network_of
            .get(&[p.0, p.1, p.2])
            .and_then(|i| solved.networks.get(*i))
        {
            s.capacity = n.capacity;
            s.stress = n.stress;
            s.status = format!("{:?}", n.state);
        }
        store_machine(game, *p, s);
    }
    for (p, name) in &positions {
        let mut rpm = solved.speeds.get(&[p.0, p.1, p.2]).copied().unwrap_or(0.0);
        if matches!(name.as_str(), "create:spout" | "create:item_drain") {
            rpm = 32.0;
        }
        let below = (p.0, p.1 - 1, p.2);
        let mut target = *p;
        if let Some(mut station) = machines::station(name) {
            if name == "create:encased_fan" {
                let face = game
                    .pack
                    .assets
                    .registry
                    .state(game.world.block(p.0, p.1, p.2))
                    .and_then(|s| s.properties.get("facing"))
                    .map(String::as_str)
                    .unwrap_or("south");
                let delta = match face {
                    "north" => [0, 0, -1],
                    "east" => [1, 0, 0],
                    "west" => [-1, 0, 0],
                    "up" => [0, 1, 0],
                    "down" => [0, -1, 0],
                    _ => [0, 0, 1],
                };
                station = match machine_name(game, (p.0 + delta[0], p.1 + delta[1], p.2 + delta[2]))
                    .as_str()
                {
                    "minecraft:water" => "splashing",
                    "minecraft:lava" => "blasting",
                    "minecraft:fire" => "smoking",
                    "minecraft:soul_fire" => "haunting",
                    _ => {
                        let mut state = read_machine(game, *p);
                        state.status = "NoMedium".into();
                        store_machine(game, *p, state);
                        continue;
                    }
                };
            }
            if matches!(
                name.as_str(),
                "create:mechanical_press" | "create:mechanical_mixer"
            ) && !game
                .world
                .block_entities
                .get(p)
                .is_some_and(|e| machines::has_inputs(&portable(e.slots())))
            {
                for dy in [1, 2] {
                    let candidate = (p.0, p.1 - dy, p.2);
                    if matches!(
                        machine_name(game, candidate).as_str(),
                        "create:basin" | "create:depot"
                    ) {
                        target = candidate;
                        if name == "create:mechanical_press"
                            && machine_name(game, candidate) == "create:basin"
                        {
                            station = "compacting";
                        }
                        break;
                    }
                }
            }
            let heat = heat_at(game, target, tick);
            if name == "create:spout"
                && machine_name(game, below) == "create:depot"
                && !game
                    .world
                    .block_entities
                    .get(p)
                    .is_some_and(|e| machines::has_inputs(&portable(e.slots())))
            {
                let _ = process_with_fluid_owner(game, below, *p, station, rpm, heat);
            } else {
                let owner = if target == *p && name == "create:mechanical_mixer" {
                    [1, 2]
                        .into_iter()
                        .map(|dy| (p.0, p.1 - dy, p.2))
                        .find(|q| machine_name(game, *q) == "create:basin")
                        .unwrap_or(target)
                } else {
                    target
                };
                let _ = process_with_fluid_owner(game, target, owner, station, rpm, heat);
                if owner != *p {
                    let progress = read_machine(game, owner);
                    let mut controller = read_machine(game, *p);
                    controller.progress = progress.progress;
                    controller.recipe = progress.recipe;
                    if !matches!(controller.status.as_str(), "Overstressed" | "Conflict") {
                        controller.status = progress.status;
                    }
                    store_machine(game, *p, controller);
                }
            }
        }

        if name == "create:mechanical_drill" && rpm != 0.0 {
            let mut data = read_machine(game, *p);
            data.progress = (data.progress + (rpm.abs() / 32.0).clamp(0.03125, 8.0)).min(20.0);
            if data.progress >= 20.0 {
                let facing = game
                    .pack
                    .assets
                    .registry
                    .state(game.world.block(p.0, p.1, p.2))
                    .and_then(|s| s.properties.get("facing"))
                    .map(String::as_str)
                    .unwrap_or("south");
                let d = match facing {
                    "north" => [0, 0, -1],
                    "east" => [1, 0, 0],
                    "west" => [-1, 0, 0],
                    "up" => [0, 1, 0],
                    "down" => [0, -1, 0],
                    _ => [0, 0, 1],
                };
                let target = [p.0 + d[0], p.1 + d[1], p.2 + d[2]];
                let state = game.world.block(target[0], target[1], target[2]);
                let f = game.res.facts().get(state);
                if f.hardness >= 0.0
                    && !f.name.starts_with("create:")
                    && !game.res.facts().is_air(state)
                    && game.world.loaded(target[0], target[2])
                {
                    if game.destroy_block_dropping(target).is_ok() {
                        data.progress = 0.0;
                    }
                }
            }
            store_machine(game, *p, data);
        }
        if name == "create:mechanical_pump" {
            if let Some(n) = nodes.get(&[p.0, p.1, p.2]) {
                pump_fluids(game, *p, n.axis, rpm)
            }
        }
        if name == "create:chute" {
            move_item(game, (p.0, p.1 + 1, p.2), *p);
            move_item(game, *p, below);
        }
    }
    tick_transport(game, &positions, &solved.speeds);
    thread_local! { static SENT: std::cell::RefCell<BTreeMap<(std::path::PathBuf,String),BTreeMap<MachinePos,String>>>=const { std::cell::RefCell::new(BTreeMap::new()) }; }
    let values: Vec<_> = positions
        .iter()
        .map(|(p, name)| MachineRecord {
            pos: [p.0, p.1, p.2],
            name: name.clone(),
            data: read_machine(game, *p),
        })
        .collect();
    let changed = SENT.with(|sent| {
        let mut sent = sent.borrow_mut();
        let old = sent
            .entry((game.save.root.clone(), format!("{:?}", game.dim)))
            .or_default();
        let mut live = std::collections::BTreeSet::new();
        let updates = values
            .into_iter()
            .filter(|r| {
                let p = (r.pos[0], r.pos[1], r.pos[2]);
                live.insert(p);
                let value = format!("{}:{}", r.name, r.data.encode());
                let changed = old.get(&p) != Some(&value);
                old.insert(p, value);
                changed || tick.is_multiple_of(5)
            })
            .collect::<Vec<_>>();
        old.retain(|p, _| live.contains(p));
        updates
    });
    broadcast_machine_state(game, &changed, "");
    finish_machine_batch(game);
}
pub(super) fn clear_removed_machines(game: &mut Game<'_>) {
    let removed: Vec<_> = game.spill.removed.iter().map(|(p, _)| *p).collect();
    for p in removed {
        let key = (p[0] >> 4, p[2] >> 4);
        let mut values = records(game, key);
        let old = values.len();
        values.retain(|r| r.pos != p);
        if values.len() != old {
            write_records(game, key, &values);
        }
    }
}
pub(super) fn use_machine(game: &mut Game<'_>, p: [i32; 3]) -> Result<bool> {
    if game.remote.is_some() || !game.may_build() {
        return Ok(false);
    }
    let pos = (p[0], p[1], p[2]);
    let name = machine_name(game, pos);
    if machines::container_size(&name).is_none() {
        return Ok(false);
    }
    ensure_machine(game, pos);
    let held = game.held().cloned();
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    if held.as_ref().is_some_and(|s| &*s.item == "create:wrench") {
        return Ok(false);
    }
    let mut state = read_machine(game, pos);
    let tick = game.world.game_time.max(0) as u64;
    let mut changed = false;
    let mut notice = String::new();
    if name == "create:creative_motor" && held.is_none() {
        if sneaking {
            state.direction = -state.direction;
        } else {
            let speeds = [0.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0];
            let i = speeds
                .iter()
                .position(|r| *r == state.motor_rpm)
                .unwrap_or(2);
            state.motor_rpm = speeds[(i + 1) % speeds.len()];
        }
        notice = format!(
            "Create motor: {} RPM",
            state.motor_rpm * f32::from(state.direction)
        );
        changed = true;
    }
    if name == "create:hand_crank" {
        state.crank_until = tick + 20;
        state.direction = if sneaking { -1 } else { 1 };
        changed = true;
    }
    if matches!(name.as_str(), "create:clutch" | "create:gearshift") && held.is_none() {
        if name.ends_with("clutch") {
            state.engaged = !state.engaged
        } else {
            state.direction = -state.direction
        }
        changed = true;
    }
    if name == "create:blaze_burner" {
        if let Some(held) = &held {
            let ticks = match &*held.item {
                "minecraft:coal" | "minecraft:charcoal" => 1600,
                "minecraft:blaze_rod" => 2400,
                "create:blaze_cake" => 3200,
                _ => 0,
            };
            if ticks > 0 {
                state.burn_until = tick + ticks;
                if &*held.item == "create:blaze_cake" {
                    state.superheat_until = tick + ticks
                }
                game.consume_held();
                changed = true;
            }
        }
    }
    if matches!(
        name.as_str(),
        "create:fluid_tank" | "create:basin" | "create:spout" | "create:item_drain"
    ) {
        if let Some(held) = &held {
            let pair = match &*held.item {
                "minecraft:water_bucket" => Some(("minecraft:water", "minecraft:bucket")),
                "minecraft:lava_bucket" => Some(("minecraft:lava", "minecraft:bucket")),
                "create:honey_bucket" => Some(("create:honey", "minecraft:bucket")),
                "create:chocolate_bucket" => Some(("create:chocolate", "minecraft:bucket")),
                "minecraft:milk_bucket" => Some(("minecraft:milk", "minecraft:bucket")),
                _ => None,
            };
            let remainder = if let Some((fluid, remainder)) = pair {
                state.insert_fluid(fluid, 1000).then_some(remainder)
            } else if &*held.item == "minecraft:bucket" {
                [
                    ("minecraft:water", "minecraft:water_bucket"),
                    ("minecraft:lava", "minecraft:lava_bucket"),
                    ("create:honey", "create:honey_bucket"),
                    ("create:chocolate", "create:chocolate_bucket"),
                    ("minecraft:milk", "minecraft:milk_bucket"),
                ]
                .into_iter()
                .find_map(|(fluid, bucket)| state.extract_fluid(fluid, 1000).then_some(bucket))
            } else {
                None
            };
            if let Some(remainder) = remainder {
                if !game.creative {
                    game.consume_held();
                    if let Some(left) = game.give(ItemStack::new(remainder, 1)) {
                        game.player_drop(left)
                    }
                }
                changed = true;
            }
        }
    }
    if changed {
        store_machine(game, pos, state.clone());
        dirty_machine(game, pos);
        broadcast_machine_state(
            game,
            &[MachineRecord {
                pos: p,
                name,
                data: state,
            }],
            &notice,
        );
        return Ok(true);
    }
    if !sneaking && machines::container_size(&name).is_some_and(|n| n >= 18) {
        game.container = Some(pos);
        game.screen = Screen::Inventory;
        game.capture(false);
        game.right = false;
        broadcast_machine_state(
            game,
            &[MachineRecord {
                pos: p,
                name,
                data: state,
            }],
            "Create machine: top row = inputs, middle row = outputs. Sneak-right-click for status.",
        );
        return Ok(true);
    }
    if sneaking {
        let text = format!(
            "Create: {:.0} RPM, {}, stress {:.0}/{:.0}, fluids {} mB",
            state.rpm,
            state.status,
            state.stress,
            state.capacity,
            state.fluid_in.values().sum::<u32>()
        );
        broadcast_machine_state(
            game,
            &[MachineRecord {
                pos: p,
                name,
                data: state,
            }],
            &text,
        );
        return Ok(true);
    }
    Ok(false)
}
pub(super) fn draw_machine_status(game: &mut Game<'_>, batch: &mut HudBatch) {
    let Some(mut pos) = game.container else {
        return;
    };
    if !machine_name(game, pos).starts_with("create:") && game.remote.is_some() {
        let Some(target) = MENU_TARGET.with(Cell::get) else {
            return;
        };
        pos = target;
    }
    let name = machine_name(game, pos);
    if !name.starts_with("create:") {
        return;
    }
    let s = read_machine(game, pos);
    let percent = s
        .recipe
        .as_deref()
        .and_then(|id| recipes::catalog::recipes().iter().find(|r| r.id == id))
        .map_or(0.0, |r| {
            (s.progress / r.work.max(1) as f32 * 100.0).clamp(0.0, 100.0)
        });
    let (x, y) = game.inventory_origin(batch);
    batch.fill(
        &game.gui.atlas,
        x,
        y - 42.0,
        176.0,
        39.0,
        rgba(25, 25, 25, 230),
    );
    batch.text(
        &game.gui.atlas,
        &format!("{:.0} RPM | {}", s.rpm, machines::status_label(&s)),
        x + 4.0,
        y - 38.0,
        u32::MAX,
        false,
    );
    batch.text(
        &game.gui.atlas,
        &format!(
            "Work {:.0}% | Fluid {} mB",
            percent,
            s.fluid_in.values().chain(s.fluid_out.values()).sum::<u32>()
        ),
        x + 4.0,
        y - 26.0,
        u32::MAX,
        false,
    );
    batch.text(
        &game.gui.atlas,
        "Inputs: top | Outputs: middle",
        x + 4.0,
        y - 14.0,
        u32::MAX,
        false,
    );
}

fn generate_zinc(game: &mut Game<'_>) {
    const MARKER: &str = "create:zinc_generated_v1";
    let overworld = game.dim == super::Dim::Overworld;
    let keys: Vec<_> = game
        .world
        .chunks
        .iter()
        .filter(|(_, c)| {
            machines::worldgen::should_generate(
                c.fresh,
                overworld,
                c.saved
                    .as_ref()
                    .is_some_and(|s| s.root.contains_key(&MARKER.into())),
            )
        })
        .map(|(k, _)| *k)
        .collect();
    let Some(ore) = game.pack.assets.registry.default_state("create:zinc_ore") else {
        return;
    };
    let Some(deep) = game
        .pack
        .assets
        .registry
        .default_state("create:deepslate_zinc_ore")
    else {
        return;
    };
    for key in keys {
        for p in machines::worldgen::zinc_positions(
            game.level.seed,
            key.0,
            key.1,
            game.world.shape.min_y,
            game.world.shape.max_y(),
        ) {
            let name = machine_name(game, (p[0], p[1], p[2]));
            let replacement = match name.as_str() {
                "minecraft:stone" => ore,
                "minecraft:deepslate" => deep,
                _ => continue,
            };
            let _ = game.world.set(p[0], p[1], p[2], replacement);
        }
        if let Some(chunk) = game.world.chunks.get_mut(&key) {
            let chunk = std::sync::Arc::make_mut(chunk);
            let saved = std::sync::Arc::make_mut(chunk.saved.get_or_insert_default());
            saved.root.insert(MARKER.into(), Tag::Int(1));
            game.world.modified.insert(key);
        }
    }
}
