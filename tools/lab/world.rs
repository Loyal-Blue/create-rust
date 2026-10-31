//! Real Horizon 26.3 save writer and read-back verifier; never opens a player save.
use create_runtime::{MachineState, kinetics as k};
use horizon_client::{containers::BlockEntity, items::ItemStack, save};
use horizon_core::{
    geometry::Direction,
    position::{BlockPos, ChunkPos},
};
use horizon_world::{
    chunk::{BlockState, ChunkData, PalettedContainer},
    compression::Compression,
    fluid::FluidState,
    fluid_flow::{FluidFlowWorld, flow_at},
    nbt::{Limits, NamedTag, Tag},
    region::{ChunkRecord, RegionFile},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};
type P = [i32; 3];
type Any = Result<(), Box<dyn std::error::Error>>;
fn strtag(s: &str) -> Tag {
    Tag::String(s.into())
}
fn p(v: &Value) -> P {
    serde_json::from_value(v.clone()).unwrap()
}
fn bs(v: &Value) -> BlockState {
    BlockState {
        name: v["name"].as_str().unwrap().into(),
        properties: serde_json::from_value(v["properties"].clone()).unwrap(),
    }
}
fn air() -> BlockState {
    BlockState {
        name: "minecraft:air".into(),
        properties: BTreeMap::new(),
    }
}
fn named(tag: Tag) -> NamedTag {
    NamedTag {
        name: "".into(),
        tag,
    }
}
struct Water<'a>(&'a BTreeMap<P, BlockState>);
impl FluidFlowWorld for Water<'_> {
    type Error = std::io::Error;
    type Block = bool;
    fn fluid(&mut self, q: BlockPos) -> Result<FluidState, Self::Error> {
        FluidState::from_block(self.0.get(&[q.x, q.y, q.z]).unwrap_or(&air()))
    }
    fn block(&mut self, q: BlockPos) -> Result<bool, Self::Error> {
        Ok(self.0.get(&[q.x, q.y, q.z]).is_some_and(|b| {
            matches!(
                b.name.as_str(),
                "minecraft:glass" | "minecraft:stone" | "minecraft:light_gray_concrete"
            )
        }))
    }
    fn blocks_fluid_flow(&self, b: bool) -> bool {
        b
    }
    fn is_ice_class(&self, _: bool) -> bool {
        false
    }
    fn is_face_sturdy(&mut self, b: bool, _: BlockPos, _: Direction) -> Result<bool, Self::Error> {
        Ok(b)
    }
}
fn networks(plan: &Value, blocks: &BTreeMap<P, BlockState>) -> Any {
    let mut nodes = BTreeMap::new();
    let registry = k::state::Registry::new();
    for m in plan["machines"].as_array().unwrap() {
        let q = p(&m["pos"]);
        let b = &blocks[&q];
        let props = b
            .properties
            .iter()
            .map(|(a, b)| format!("{a}={b}"))
            .collect::<Vec<_>>()
            .join(",");
        if let Some(mut n) = k::state::parse_state(&registry, &format!("{}[{props}]", b.name)) {
            let state = MachineState::decode(&m["data"].to_string()).unwrap();
            n.direction = if n.kind == k::Kind::Belt {
                n.direction * state.direction
            } else {
                state.direction
            };
            match n.kind {
                k::Kind::CreativeMotor => n.configured_rpm = Some(state.motor_rpm),
                k::Kind::HandCrank => n.active = true,
                k::Kind::WaterWheel | k::Kind::LargeWaterWheel => {
                    let r: i32 = if n.kind == k::Kind::LargeWaterWheel {
                        2
                    } else {
                        1
                    };
                    let mut flow = vec![];
                    let mut water = Water(blocks);
                    for u in -r..=r {
                        for v in -r..=r {
                            if u.abs() != r && v.abs() != r {
                                continue;
                            }
                            let d = match n.axis {
                                k::Axis::X => [0, u, v],
                                k::Axis::Y => [u, 0, v],
                                k::Axis::Z => [u, v, 0],
                            };
                            let pos = BlockPos::new(q[0] + d[0], q[1] + d[1], q[2] + d[2]);
                            let f = water.fluid(pos)?;
                            let vel = flow_at(&mut water, f, pos)?;
                            flow.push(create_horizon_core::FlowSample {
                                offset: d.map(f64::from),
                                velocity: [vel.x, vel.y, vel.z],
                            });
                        }
                    }
                    let axis = match n.axis {
                        k::Axis::X => create_horizon_core::Axis::X,
                        k::Axis::Y => create_horizon_core::Axis::Y,
                        k::Axis::Z => create_horizon_core::Axis::Z,
                    };
                    let rpm = create_horizon_core::wheel_rpm(axis, &flow);
                    n.active = rpm != 0.;
                    n.direction = if rpm < 0. { -1 } else { 1 };
                    if plan["cases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|c| c.get("wheel").is_some_and(|v| p(v) == q))
                    {
                        assert!(n.active, "dry wheel at {q:?}");
                        println!("Wheel {q:?}: nonzero torque from Horizon falling-water flow");
                    }
                }
                _ => {}
            }
            nodes.insert(q, n);
        }
    }
    let solution = k::solve(&nodes);
    for c in plan["cases"].as_array().unwrap() {
        for t in c["targets"].as_array().unwrap() {
            let q = p(&t["pos"]);
            let want = t["rpm"].as_f64().unwrap() as f32;
            let got = solution.speeds.get(&q).copied().unwrap_or_default();
            assert!(
                (got - want).abs() < 0.01,
                "Bay {} {} target {q:?}: got {got}, expected {want}",
                c["id"],
                c["title"]
            );
        }
        if c["kind"] == "conflict" {
            let q = p(&c["targets"][0]["pos"]);
            assert_eq!(
                solution.networks[solution.network_of[&q]].state,
                k::State::Conflict
            );
        }
        if c["kind"] == "stress" {
            let q = p(&c["crank"]);
            assert_eq!(
                solution.networks[solution.network_of[&q]].state,
                k::State::Overstressed
            );
        }
    }
    // The initial saved ON levers disengage all drive clutches. Gearshift levers stay OFF.
    let mut paused = nodes.clone();
    for n in paused.values_mut() {
        if n.kind == k::Kind::Clutch || n.kind == k::Kind::HandCrank {
            n.active = false;
        }
    }
    let s = k::solve(&paused);
    for c in plan["cases"].as_array().unwrap() {
        if c["kind"] == "conflict" {
            continue;
        }
        for t in c["targets"].as_array().unwrap() {
            let q = p(&t["pos"]);
            assert_eq!(
                s.speeds.get(&q).copied().unwrap_or_default(),
                0.,
                "Bay {} not paused",
                c["id"]
            );
        }
    }
    println!(
        "PASS: all labelled drive RPMs, source conflict, crank overstress, paused loads and both water flows"
    );
    Ok(())
}
fn processing(plan: &Value) -> Any {
    let containers: BTreeMap<P, &Value> = plan["containers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (p(&v["pos"]), v))
        .collect();
    let records: BTreeMap<P, &Value> = plan["machines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (p(&v["pos"]), v))
        .collect();
    let mut checked = 0;
    for case in plan["cases"].as_array().unwrap() {
        let Some(id) = case.get("recipe_id").and_then(Value::as_str) else {
            continue;
        };
        // Vanilla fan cooking is resolved by Horizon's installed cooking catalogue.
        let Some(recipe) = create_simulation::active_recipes()
            .iter()
            .find(|r| r.id == id)
        else {
            continue;
        };
        let origin = p(&case["origin"]);
        let (&q, c) = containers
            .iter()
            .find(|(q, v)| {
                q[0] >= origin[0]
                    && q[0] < origin[0] + 24
                    && q[2] >= origin[2]
                    && q[2] < origin[2] + 24
                    && v["kind"] == "chest"
                    && records.get(*q).is_some_and(|m| {
                        matches!(
                            m["name"].as_str(),
                            Some("create:basin" | "create:depot" | "create:millstone")
                        )
                    })
            })
            .expect("prefilled recipe container");
        let mut slots = vec![None; 27];
        for item in c["items"].as_array().unwrap() {
            slots[item["slot"].as_u64().unwrap() as usize] = Some(create_runtime::Stack {
                id: item["id"].as_str().unwrap().into(),
                count: item["count"].as_u64().unwrap() as u32,
                components: String::new(),
            });
        }
        let mut state = MachineState::decode(&records[&q]["data"].to_string()).unwrap();
        let mut done = false;
        for _ in 0..10000 {
            let u = create_runtime::process_with_catalog(
                &state,
                &slots,
                recipe.station,
                32.,
                recipe.heat,
                std::slice::from_ref(recipe),
                |_| 64,
            )?;
            state = u.state;
            assert!(u.inventory.commit(&mut slots));
            if u.result == create_runtime::TickResult::Completed {
                done = true;
                break;
            }
        }
        assert!(
            done,
            "Bay {} seeded recipe did not complete: {id}",
            case["id"]
        );
        assert!(slots[9..18].iter().any(Option::is_some) || !state.fluid_out.is_empty());
        checked += 1;
    }
    for case in plan["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("crafting_grid").is_some())
    {
        let origin = p(&case["origin"]);
        let nodes: BTreeMap<_, _> = records
            .iter()
            .filter(|(q, m)| {
                q[0] >= origin[0]
                    && q[0] < origin[0] + 24
                    && q[2] >= origin[2]
                    && q[2] < origin[2] + 24
                    && m["name"] == "create:mechanical_crafter"
            })
            .map(|(&q, _)| (q, 0))
            .collect();
        let grids = create_runtime::crafting::grids(&nodes);
        assert_eq!(grids.len(), 1);
        let grid = &grids[0];
        assert_eq!((grid.width, grid.height), (5, 5));
        for (q, ch) in grid.cells.iter().zip(
            [" AAA ", "AAPAA", "APSPA", "AAPAA", " AAA "]
                .concat()
                .chars(),
        ) {
            let items = containers[q]["items"].as_array().unwrap();
            if ch == ' ' {
                assert!(items.is_empty());
            } else {
                assert_eq!(items.len(), 1);
                let id = match ch {
                    'A' => "create:andesite_alloy",
                    'P' => "minecraft:oak_planks",
                    _ => "minecraft:stone",
                };
                assert_eq!(items[0]["id"], id);
                assert_eq!(items[0]["count"], 2);
            }
        }
    }
    println!(
        "PASS: {checked} prefilled recipes complete transactionally; crafter grid and two complete input patterns match"
    );
    Ok(())
}
fn catalog(out: &Path) -> Any {
    let recipes=create_simulation::active_recipes().iter().map(|r|json!({"id":r.id,"station":r.station,"heat":format!("{:?}",r.heat),"ingredients":r.ingredients.iter().map(|i|json!({"matcher":i.matcher,"count":i.count,"fluid":i.fluid,"consume":i.consume})).collect::<Vec<_>>(),"products":r.products.iter().map(|i|json!({"id":i.id,"count":i.count,"fluid":i.fluid,"chance":i.chance})).collect::<Vec<_>>()})).collect::<Vec<_>>();
    let tags: BTreeMap<_, _> = create_simulation::catalog::tags().iter().copied().collect();
    fs::write(
        out,
        serde_json::to_vec_pretty(&json!({"recipes":recipes,"tags":tags}))?,
    )?;
    Ok(())
}
fn build(input: &Path, out: &Path) -> Any {
    assert!(!out.exists(), "Refusing to overwrite an existing world");
    let plan: Value = serde_json::from_slice(&fs::read(input)?)?;
    let authored: BTreeMap<P, BlockState> = plan["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (p(&v["pos"]), bs(v)))
        .collect();
    networks(&plan, &authored)?;
    processing(&plan)?;
    let mut all = BTreeMap::new();
    let w = plan["width"].as_i64().unwrap() as i32;
    let l = plan["length"].as_i64().unwrap() as i32;
    for x in 0..w {
        for z in 0..l {
            for (y, name) in [
                (62, "minecraft:bedrock"),
                (63, "minecraft:stone"),
                (
                    64,
                    if x % 8 == 0 && z % 8 == 0 {
                        "minecraft:sea_lantern"
                    } else {
                        "minecraft:light_gray_concrete"
                    },
                ),
            ] {
                all.insert(
                    [x, y, z],
                    BlockState {
                        name: name.into(),
                        properties: BTreeMap::new(),
                    },
                );
            }
            if x == 0 || z == 0 || x == w - 1 || z == l - 1 {
                for y in 65..69 {
                    all.insert(
                        [x, y, z],
                        BlockState {
                            name: "minecraft:glass".into(),
                            properties: BTreeMap::new(),
                        },
                    );
                }
            }
        }
    }
    all.extend(authored.clone());
    let mut sections: BTreeMap<(i32, i32), BTreeMap<i32, PalettedContainer>> = BTreeMap::new();
    for (&[x, y, z], b) in &all {
        let section = sections
            .entry((x.div_euclid(16), z.div_euclid(16)))
            .or_default()
            .entry(y.div_euclid(16))
            .or_insert_with(|| PalettedContainer::uniform(air().to_nbt(), 4096, 4).unwrap());
        section.set(
            ((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize,
            b.to_nbt(),
        )?;
    }
    let mut containers: BTreeMap<P, Value> = plan["containers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (p(&v["pos"]), v.clone()))
        .collect();
    let mut machine_tags: BTreeMap<(i32, i32), Vec<Value>> = BTreeMap::new();
    for m in plan["machines"].as_array().unwrap() {
        let q = p(&m["pos"]);
        let state = MachineState::decode(&m["data"].to_string()).unwrap();
        let mut record = m.clone();
        record["data"] = serde_json::from_str(&state.encode())?;
        machine_tags
            .entry((q[0].div_euclid(16), q[2].div_euclid(16)))
            .or_default()
            .push(record);
        if create_runtime::container_size(m["name"].as_str().unwrap()).is_some_and(|n| n > 0) {
            containers
                .entry(q)
                .or_insert_with(|| json!({"pos":q,"kind":"chest","items":[]}));
        }
    }
    let mut entities: BTreeMap<(i32, i32), Vec<Tag>> = BTreeMap::new();
    for (&q, c) in &containers {
        let tag = if c["kind"] == "sign" {
            let mut lines = c["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| Tag::compound([("text", strtag(s.as_str().unwrap()))]))
                .collect::<Vec<_>>();
            while lines.len() < 4 {
                lines.push(Tag::compound([("text", strtag(""))]));
            }
            let text = Tag::compound([
                ("messages", Tag::List(lines)),
                ("color", strtag("black")),
                ("has_glowing_text", Tag::Byte(1)),
            ]);
            Tag::compound([
                ("id", strtag("minecraft:sign")),
                ("x", Tag::Int(q[0])),
                ("y", Tag::Int(q[1])),
                ("z", Tag::Int(q[2])),
                ("front_text", text.clone()),
                ("back_text", text),
                ("is_waxed", Tag::Byte(1)),
            ])
        } else {
            let mut slots = vec![None; if c["kind"] == "dropper" { 9 } else { 27 }];
            for s in c["items"].as_array().unwrap() {
                let id = s["id"].as_str().unwrap();
                let count = s["count"].as_u64().unwrap() as u32;
                let stack = if let Some(name) = s.get("name") {
                    let text = json!({"text":name.as_str().unwrap()}).to_string();
                    horizon_client::item_data::parse_stack(&format!(
                        "{{id:{},count:{count},components:{{\"minecraft:custom_name\":{text}}}}}",
                        serde_json::to_string(id)?
                    ))
                    .expect("valid named item")
                    .expect("nonempty named item")
                } else {
                    ItemStack::new(id, count)
                };
                slots[s["slot"].as_u64().unwrap() as usize] = Some(stack);
            }
            save::encode_block_entity(
                (q[0], q[1], q[2]),
                &BlockEntity::Chest(slots),
                &authored[&q].name,
            )
        };
        let (saved_pos, decoded) =
            save::decode_block_entity(&tag).expect("saved container/sign decodes");
        assert_eq!(saved_pos, (q[0], q[1], q[2]));
        if c["kind"] != "sign" {
            for s in c["items"].as_array().unwrap() {
                let stack = decoded.slots()[s["slot"].as_u64().unwrap() as usize]
                    .as_ref()
                    .unwrap();
                assert_eq!(&*stack.item, s["id"].as_str().unwrap());
                assert_eq!(stack.count, s["count"].as_u64().unwrap() as u32);
                if s.get("name").is_some() {
                    assert!(
                        stack.data().is_some_and(|d| d.custom_name.is_some()),
                        "lost item name"
                    );
                }
            }
        }
        entities
            .entry((q[0].div_euclid(16), q[2].div_euclid(16)))
            .or_default()
            .push(tag);
    }
    fs::create_dir_all(out.join("region"))?;
    let mut regions: BTreeMap<(i32, i32), RegionFile> = BTreeMap::new();
    let mut count = 0;
    for ((x, z), mut palettes) in sections {
        let mut list = vec![];
        for y in -4..20 {
            let b =
                palettes
                    .remove(&y)
                    .unwrap_or(PalettedContainer::uniform(air().to_nbt(), 4096, 4)?);
            let biome = PalettedContainer::uniform(strtag("minecraft:plains"), 64, 1)?;
            list.push(Tag::compound([
                ("Y", Tag::Byte(y as i8)),
                ("block_states", b.to_nbt()),
                ("biomes", biome.to_nbt()),
            ]));
        }
        let root = named(Tag::compound([
            ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
            ("xPos", Tag::Int(x)),
            ("zPos", Tag::Int(z)),
            ("yPos", Tag::Int(-4)),
            ("Status", strtag("minecraft:full")),
            ("sections", Tag::List(list)),
            (
                "block_entities",
                Tag::List(entities.remove(&(x, z)).unwrap_or_default()),
            ),
            ("block_ticks", Tag::List(vec![])),
            ("fluid_ticks", Tag::List(vec![])),
            ("Heightmaps", Tag::compound([] as [(&str, Tag); 0])),
            ("isLightOn", Tag::Byte(0)),
            ("LastUpdate", Tag::Long(0)),
            ("InhabitedTime", Tag::Long(0)),
            (
                "create:machines",
                strtag(&serde_json::to_string(
                    &machine_tags.remove(&(x, z)).unwrap_or_default(),
                )?),
            ),
        ]));
        let chunk = ChunkData::new(root.clone())?;
        for (&q, b) in authored
            .iter()
            .filter(|(q, _)| q[0].div_euclid(16) == x && q[2].div_euclid(16) == z)
        {
            assert_eq!(
                chunk.block(BlockPos::new(q[0], q[1], q[2]))?.as_ref(),
                Some(b)
            );
        }
        let r = (x.div_euclid(32), z.div_euclid(32));
        if !regions.contains_key(&r) {
            regions.insert(
                r,
                RegionFile::open_writable(out.join(format!("region/r.{}.{}.mca", r.0, r.1)), true)?,
            );
        }
        regions.get_mut(&r).unwrap().write(
            ChunkPos::new(x, z),
            &ChunkRecord::from_nbt(&root, Compression::Zlib, 0, Limits::saved())?,
        )?;
        count += 1;
    }
    for r in regions.values() {
        r.sync()?;
    }
    drop(regions);
    let mut checked = 0;
    for entry in fs::read_dir(out.join("region"))? {
        let mut r = RegionFile::open(entry?.path())?;
        let chunks: Vec<_> = r.chunks().collect();
        for (q, _) in chunks {
            let n = r.read(q)?.unwrap().nbt(Limits::saved())?;
            let chunk = ChunkData::new(n.clone())?;
            for (&pos, b) in authored
                .iter()
                .filter(|(p, _)| p[0].div_euclid(16) == q.x && p[2].div_euclid(16) == q.z)
            {
                assert_eq!(
                    chunk.block(BlockPos::new(pos[0], pos[1], pos[2]))?.as_ref(),
                    Some(b)
                );
            }
            if let Some(Tag::List(v)) = n.tag.get("block_entities") {
                for e in v {
                    assert!(save::decode_block_entity(e).is_some());
                }
            }
            if let Some(Tag::String(v)) = n.tag.get("create:machines") {
                let records: Vec<Value> = serde_json::from_str(&v.to_string()?)?;
                for m in records {
                    MachineState::decode(&m["data"].to_string()).unwrap();
                    assert_eq!(authored[&p(&m["pos"])].name, m["name"].as_str().unwrap());
                }
            }
            checked += 1;
        }
    }
    assert_eq!(count, checked);
    let spawn = p(&plan["spawn"]);
    let mut level = save::Level::default();
    level.name = plan["name"].as_str().unwrap().into();
    level.creative = true;
    level.game_mode = 1;
    level.position = Some([
        f64::from(spawn[0]) + 0.5,
        f64::from(spawn[1]),
        f64::from(spawn[2]) + 0.5,
    ]);
    level.world_spawn = Some(spawn);
    level.rotation = [0., 0.];
    level.difficulty = "peaceful".into();
    level.allow_commands = Some(true);
    level.health = 20.;
    level.food = 20;
    level.day_time = 6000;
    for (name, v) in [
        ("advance_time", 0),
        ("advance_weather", 0),
        ("spawn_mobs", 0),
        ("random_tick_speed", 0),
        ("keep_inventory", 1),
        ("fire_spread_radius_around_player", 0),
    ] {
        level.game_rules.insert(name.into(), v);
    }
    level.inventory = vec![
        (0, "create:wrench".into(), 1, 0),
        (1, "minecraft:diamond_pickaxe".into(), 1, 0),
    ];
    level.write(out)?;
    let layer = |block: &str, height| {
        Tag::compound([("block", strtag(block)), ("height", Tag::Int(height))])
    };
    let flat = Tag::compound([
        ("biome", strtag("minecraft:plains")),
        ("features", Tag::Byte(0)),
        ("lakes", Tag::Byte(0)),
        (
            "layers",
            Tag::List(vec![
                layer("minecraft:bedrock", 1),
                layer("minecraft:stone", 127),
                layer("minecraft:light_gray_concrete", 1),
            ]),
        ),
        ("structure_overrides", Tag::List(vec![])),
    ]);
    save::write_world_gen_settings_with(
        out,
        263,
        save::WorldGen {
            preset: save::WorldPreset::Flat,
            structures: false,
            bonus_chest: false,
        },
        Some(flat),
    )?;
    let read = save::Level::read(out).expect("level roundtrip");
    assert_eq!(read.world_spawn, Some(spawn));
    assert!(read.creative);
    assert_eq!(read.name, level.name);
    fs::copy(input, out.join("test-plan.json"))?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &json!({"chunks":checked,"authored_blocks":authored.len(),"machine_records":plan["machines"].as_array().unwrap().len(),"containers_and_signs":containers.len(),"bays":plan["cases"].as_array().unwrap().len(),"checks":["Horizon compressed-region roundtrip","all authored block states","block entity and inventory codec","machine metadata codec","creative level and spawn roundtrip","drive RPM and paused loads","conflict and overstress networks","actual falling-water flow torque","prefilled recipe transactions and crafter grid"],"gameplay_walkthrough":false}),
        )?,
    )?;
    println!(
        "PASS: {checked} compressed chunks, all blocks/containers/machine records and creative level metadata roundtrip"
    );
    Ok(())
}
fn main() -> Any {
    let a: Vec<_> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("catalog") => catalog(Path::new(&a[2])),
        Some("build") => build(Path::new(&a[2]), Path::new(&a[3])),
        _ => panic!("catalog OUTPUT | build PLAN WORLD"),
    }
}
