// Minimal isolated host model around the unmodified production adapter. Item
// stacks, components, reservations, recipes and controller code are real.
use horizon_core::position::BlockPos;
use machines::surface;
use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
};
type MachinePos = (i32, i32, i32);
#[derive(Clone, Copy, Default)]
struct V {
    x: f64,
    y: f64,
    z: f64,
}
struct Entity {
    id: u32,
    stack: ItemStack,
    position: V,
    velocity: V,
    on_ground: bool,
    target: Option<()>,
}
struct World {
    block_entities: BTreeMap<MachinePos, Be>,
    names: BTreeMap<MachinePos, String>,
    game_time: i64,
    paused: bool,
}
type Be = TestChest;
#[derive(Clone)]
struct TestChest(Vec<Option<ItemStack>>);
impl TestChest {
    fn slots(&self) -> &[Option<ItemStack>] {
        &self.0
    }
    fn slots_mut(&mut self) -> &mut [Option<ItemStack>] {
        &mut self.0
    }
}
impl World {
    fn loaded(&self, _: i32, _: i32) -> bool {
        true
    }
    fn block(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.names.get(&(x, y, z)).map(String::as_str) {
            None => 0,
            Some("minecraft:stone") => 1,
            _ => 2,
        }
    }
}
struct Fact {
    full_collision: bool,
}
struct Facts;
struct Res;
impl Res {
    fn facts(&self) -> Facts {
        Facts
    }
}
impl Facts {
    fn get(&self, id: u8) -> Fact {
        Fact {
            full_collision: id == 1,
        }
    }
    fn is_air(&self, id: u8) -> bool {
        id == 0
    }
}
struct Item {
    max_stack: u32,
}
struct Items;
impl Items {
    fn get(&self, id: &str) -> Option<Item> {
        Some(Item {
            max_stack: if id.ends_with("_axe") { 1 } else { 64 },
        })
    }
}
struct Behaviors;
struct Level {
    paused: bool,
}
impl Behaviors {
    fn has_neighbor_signal(&self, l: &mut Level, _: &mut (), _: BlockPos) -> bool {
        l.paused
    }
}
struct Game<'a> {
    world: World,
    res: Res,
    dropped: Vec<Entity>,
    next_entity: u32,
    states: BTreeMap<MachinePos, machines::MachineState>,
    facing: [i32; 3],
    _l: PhantomData<&'a ()>,
}
impl Game<'_> {
    fn with_level<R>(&mut self, f: impl FnOnce(&Behaviors, &mut Level, &mut ()) -> R) -> R {
        f(
            &Behaviors,
            &mut Level {
                paused: self.world.paused,
            },
            &mut (),
        )
    }
    fn items(&self) -> Items {
        Items
    }
    fn spawn_item(&mut self, stack: ItemStack, position: [f64; 3], velocity: [f64; 3], _: u32) {
        self.next_entity += 1;
        self.dropped.push(Entity {
            id: self.next_entity,
            stack,
            position: V {
                x: position[0],
                y: position[1],
                z: position[2],
            },
            velocity: V {
                x: velocity[0],
                y: velocity[1],
                z: velocity[2],
            },
            on_ground: true,
            target: None,
        });
    }
}
#[derive(Default)]
struct SurfaceRun {
    held: BTreeSet<u32>,
}
fn read_machine(g: &Game<'_>, p: MachinePos) -> machines::MachineState {
    g.states.get(&p).cloned().unwrap_or_default()
}
fn store_machine(g: &mut Game<'_>, p: MachinePos, s: machines::MachineState) {
    g.states
        .insert(p, machines::MachineState::decode(&s.encode()).unwrap());
}
fn dirty_machine(_: &mut Game<'_>, _: MachinePos) {}
fn machine_name(g: &Game<'_>, p: MachinePos) -> String {
    g.world
        .names
        .get(&p)
        .cloned()
        .unwrap_or("minecraft:air".into())
}
fn facing_step(g: &Game<'_>, _: MachinePos) -> [i32; 3] {
    g.facing
}
fn offset_machine(p: MachinePos, d: [i32; 3], n: i32) -> MachinePos {
    (p.0 + d[0] * n, p.1 + d[1] * n, p.2 + d[2] * n)
}
fn clear_world_work(g: &mut Game<'_>, p: MachinePos) {
    let mut s = read_machine(g, p);
    if s.world_target.take().is_some() {
        s.progress = 0.;
        s.recipe = None;
        store_machine(g, p, s);
    }
}
fn surface_catalog(_: &Game<'_>, station: &str) -> Vec<recipes::Recipe> {
    recipes::active_recipes()
        .iter()
        .filter(|r| r.station == station)
        .copied()
        .collect()
}
fn scene(depot: bool) -> Game<'static> {
    let p = (0, 67, 0);
    let q = (0, 65, 0);
    let mut g = Game {
        world: World {
            block_entities: BTreeMap::new(),
            names: BTreeMap::new(),
            game_time: 0,
            paused: false,
        },
        res: Res,
        dropped: vec![],
        next_entity: 10,
        states: BTreeMap::new(),
        facing: [0, -1, 0],
        _l: PhantomData,
    };
    g.world.names.insert(p, "create:deployer".into());
    g.world
        .names
        .insert(q, if depot { "create:depot" } else { "create:belt" }.into());
    let mut hand = vec![None; 27];
    hand[0] = Some(ItemStack::new("minecraft:oak_planks", 64));
    g.world.block_entities.insert(p, TestChest(hand));
    if depot {
        let mut slots = vec![None; 27];
        slots[0] = Some(ItemStack::new("create:shaft", 3));
        g.world.block_entities.insert(q, TestChest(slots));
    } else {
        g.dropped.push(Entity {
            id: 7,
            stack: ItemStack::new("create:shaft", 1),
            position: V {
                x: 0.5,
                y: 65.8125,
                z: 0.5,
            },
            velocity: V::default(),
            on_ground: true,
            target: None,
        });
    }
    g
}
fn ticks(g: &mut Game<'_>, n: usize, rpm: f32) {
    for _ in 0..n {
        g.world.game_time += 1;
        tick_deployer_application(g, (0, 67, 0), rpm, &mut SurfaceRun::default());
    }
}
fn count(g: &Game<'_>, id: &str) -> u32 {
    g.dropped
        .iter()
        .filter(|e| &*e.stack.item == id)
        .map(|e| e.stack.count)
        .sum()
}
#[test]
fn depot_batch_commits_both_inventories() {
    let mut g = scene(true);
    ticks(&mut g, 200, 32.);
    let depot = &g.world.block_entities[&(0, 65, 0)].0;
    assert!(depot[0].is_none());
    assert_eq!(depot[9].as_ref().unwrap().count, 3);
    assert_eq!(&*depot[9].as_ref().unwrap().item, "create:cogwheel");
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        61
    );
}
#[test]
fn belt_output_is_not_immediately_upgraded_to_large_cog() {
    let mut g = scene(false);
    ticks(&mut g, 200, 32.);
    assert_eq!(count(&g, "create:cogwheel"), 1);
    assert_eq!(count(&g, "create:large_cogwheel"), 0);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        63
    );
}
#[test]
fn bulk_belt_outputs_are_all_guarded() {
    let mut g = scene(false);
    g.dropped[0].stack.count = 3;
    ticks(&mut g, 400, 32.);
    assert_eq!(count(&g, "create:cogwheel"), 3);
    assert_eq!(count(&g, "create:large_cogwheel"), 0);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        61
    );
}
#[test]
fn full_depot_resumes_without_consuming_while_blocked() {
    let mut g = scene(true);
    for s in &mut g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[9..18] {
        *s = Some(ItemStack::new("minecraft:dirt", 64));
    }
    ticks(&mut g, 100, 32.);
    assert_eq!(
        g.world.block_entities[&(0, 65, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        3
    );
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        64
    );
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[9] = None;
    ticks(&mut g, 200, 32.);
    assert!(g.world.block_entities[&(0, 65, 0)].0[0].is_none());
    assert_eq!(
        g.world.block_entities[&(0, 65, 0)].0[9]
            .as_ref()
            .unwrap()
            .count,
        3
    );
}
#[test]
fn redstone_pause_keeps_items_then_resumes() {
    let mut g = scene(false);
    g.world.paused = true;
    ticks(&mut g, 100, 32.);
    assert_eq!(count(&g, "create:shaft"), 1);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        64
    );
    g.world.paused = false;
    ticks(&mut g, 100, 32.);
    assert_eq!(count(&g, "create:cogwheel"), 1);
}
#[test]
fn power_and_obstructions_prevent_world_consumption() {
    let mut g = scene(false);
    ticks(&mut g, 100, 0.);
    assert_eq!(count(&g, "create:shaft"), 1);
    g.world.names.insert((0, 66, 0), "minecraft:stone".into());
    ticks(&mut g, 100, 32.);
    assert_eq!(count(&g, "create:shaft"), 1);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        64
    );
}
#[test]
fn retained_actual_named_damaged_axe_survives_depot_batch() {
    let mut g = scene(true);
    let axe=horizon_client::item_data::parse_stack(r#"{id:"minecraft:iron_axe",count:1,components:{"minecraft:damage":7,"minecraft:custom_name":{text:"Test axe"}}}"#).unwrap().unwrap();
    g.world.block_entities.get_mut(&(0, 67, 0)).unwrap().0[0] = Some(axe.clone());
    g.world.block_entities.get_mut(&(0, 65, 0)).unwrap().0[0] =
        Some(ItemStack::new("minecraft:exposed_copper", 2));
    ticks(&mut g, 200, 32.);
    assert_eq!(g.world.block_entities[&(0, 67, 0)].0[0], Some(axe));
    assert_eq!(
        &*g.world.block_entities[&(0, 65, 0)].0[9]
            .as_ref()
            .unwrap()
            .item,
        "minecraft:copper_block"
    );
    assert_eq!(
        g.world.block_entities[&(0, 65, 0)].0[9]
            .as_ref()
            .unwrap()
            .count,
        2
    );
}
#[test]
fn leaving_work_area_allows_a_new_pass() {
    let mut g = scene(false);
    ticks(&mut g, 40, 32.);
    assert_eq!(count(&g, "create:cogwheel"), 1);
    g.dropped[0].position.x = 2.;
    ticks(&mut g, 1, 32.);
    g.dropped[0].position.x = 0.5;
    ticks(&mut g, 100, 32.);
    assert_eq!(count(&g, "create:large_cogwheel"), 1);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        62
    );
}
#[test]
fn horizontal_reach_finds_surface_two_blocks_ahead_and_one_below() {
    let mut g = scene(false);
    g.facing = [0, 0, -1];
    g.world.names.insert((0, 66, -2), "create:belt".into());
    g.dropped[0].position = V {
        x: 0.5,
        y: 66.8125,
        z: -1.5,
    };
    ticks(&mut g, 100, 32.);
    assert_eq!(count(&g, "create:cogwheel"), 1);
    assert_eq!(
        g.world.block_entities[&(0, 67, 0)].0[0]
            .as_ref()
            .unwrap()
            .count,
        63
    );
}
