//! Real Horizon API 0.3 server mod. Content is a separate API 0.2 package.
//! Machine stocks are a creative laboratory, never player inventories.
use create_kinetics::{
    Kind, Node, Pos, Solution, solve,
    state::{Registry, parse_state},
};
use create_simulation::{Heat, Machine, TickResult, catalog};
use horizon_mod_sdk::{
    server::{Args, Argument, BlockPos, GameMode, Player, Registration, ServerMod, players, world},
    storage,
};
use std::collections::{BTreeMap, VecDeque};

const MAX_NODES: usize = 512;
const MAX_SCAN: i32 = 12;
const SCAN_BUDGET: usize = 256;
const MAX_LABS: usize = 16;
type Key = (String, Pos);
#[derive(Clone, Debug)]
struct Record {
    kind: Kind,
    direction: i8,
    engaged: bool,
}
struct Lab {
    at: Key,
    machine: Machine,
    last: TickResult,
}
struct Scan {
    dimension: String,
    centre: Pos,
    radius: i32,
    cursor: usize,
    found: usize,
    player: String,
}
struct CreateMod {
    registry: Registry,
    records: BTreeMap<Key, Record>,
    live: BTreeMap<String, BTreeMap<Pos, Node>>,
    solutions: BTreeMap<String, Solution>,
    cranks: BTreeMap<Key, u64>,
    scans: VecDeque<Scan>,
    labs: BTreeMap<String, Lab>,
    // No world ID in API 0.3: persistence uses an explicit /create_slot.
    slot: Option<String>,
    tick: u64,
    dirty_save: bool,
}
fn p(pos: BlockPos) -> Pos {
    [pos.x, pos.y, pos.z]
}
fn feet(player: &Player) -> Pos {
    [
        player.x.floor() as i32,
        (player.y.floor() as i32).saturating_sub(1),
        player.z.floor() as i32,
    ]
}
fn creative(player: &Player) -> Result<(), String> {
    if matches!(player.game_mode, GameMode::Creative) {
        Ok(())
    } else {
        Err("This alpha's setup and laboratory commands require Creative mode".into())
    }
}
fn station_matches(station: &str, kind: Kind) -> bool {
    match station {
        "milling" => kind == Kind::Millstone,
        "pressing" | "compacting" => kind == Kind::MechanicalPress,
        "mixing" => kind == Kind::MechanicalMixer,
        "cutting" => kind == Kind::MechanicalSaw,
        "crushing" => kind == Kind::CrushingWheel,
        "deploying" => kind == Kind::Deployer,
        "splashing" | "haunting" => kind == Kind::EncasedFan,
        _ => false,
    }
}
impl CreateMod {
    fn record(&mut self, dim: &str, pos: Pos, state: &str) -> bool {
        let key = (dim.into(), pos);
        let Some(n) = parse_state(&self.registry, state) else {
            if self.records.remove(&key).is_some() {
                self.dirty_save = true;
            }
            self.cranks.remove(&key);
            return false;
        };
        if let Some(old) = self.records.get(&key) {
            if old.kind == n.kind {
                return true;
            }
        } else if self.records.len() >= MAX_NODES {
            return false;
        }
        self.records.insert(
            key,
            Record {
                kind: n.kind,
                direction: 1,
                engaged: true,
            },
        );
        self.dirty_save = true;
        true
    }
    fn learn(&mut self, dim: &str, pos: Pos) -> bool {
        match world::get_block(dim, pos[0], pos[1], pos[2]) {
            Some(s) => self.record(dim, pos, &s),
            None => false,
        }
    }
    /// Unloaded blocks are remembered but contribute no speed or capacity.
    fn refresh(&mut self) {
        let mut next: BTreeMap<String, BTreeMap<Pos, Node>> = BTreeMap::new();
        let mut stale = Vec::new();
        for ((dim, pos), record) in &self.records {
            let Some(state) = world::get_block(dim, pos[0], pos[1], pos[2]) else {
                continue;
            };
            let Some(mut node) = parse_state(&self.registry, &state) else {
                stale.push((dim.clone(), *pos));
                continue;
            };
            if node.kind != record.kind {
                stale.push((dim.clone(), *pos));
                continue;
            }
            node.direction *= record.direction;
            if node.kind == Kind::Clutch {
                node.active &= record.engaged;
            }
            if node.kind == Kind::HandCrank {
                node.active = self
                    .cranks
                    .get(&(dim.clone(), *pos))
                    .is_some_and(|end| *end > self.tick);
            }
            if matches!(node.kind, Kind::WaterWheel | Kind::LargeWaterWheel) {
                node.active = [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ]
                .into_iter()
                .filter(|d| d[node.axis.index()] == 0)
                .any(|d| {
                    let Some(x) = pos[0].checked_add(d[0]) else {
                        return false;
                    };
                    let Some(y) = pos[1].checked_add(d[1]) else {
                        return false;
                    };
                    let Some(z) = pos[2].checked_add(d[2]) else {
                        return false;
                    };
                    world::get_block(dim, x, y, z)
                        .is_some_and(|s| s.split('[').next() == Some("minecraft:water"))
                });
            }
            next.entry(dim.clone()).or_default().insert(*pos, node);
        }
        for key in stale {
            self.records.remove(&key);
            self.cranks.remove(&key);
            self.dirty_save = true;
        }
        if next != self.live {
            self.solutions = next
                .iter()
                .map(|(dim, nodes)| (dim.clone(), solve(nodes)))
                .collect();
            self.live = next;
        }
    }
    fn scan_tick(&mut self) {
        let Some(mut scan) = self.scans.pop_front() else {
            return;
        };
        let side = (scan.radius * 2 + 1) as usize;
        let total = side * side * side;
        let end = (scan.cursor + SCAN_BUDGET).min(total);
        while scan.cursor < end {
            let i = scan.cursor;
            let d = [
                (i / (side * side)) as i32 - scan.radius,
                ((i / side) % side) as i32 - scan.radius,
                (i % side) as i32 - scan.radius,
            ];
            if let (Some(x), Some(y), Some(z)) = (
                scan.centre[0].checked_add(d[0]),
                scan.centre[1].checked_add(d[1]),
                scan.centre[2].checked_add(d[2]),
            ) && self.learn(&scan.dimension, [x, y, z])
            {
                scan.found += 1;
            }
            scan.cursor += 1;
        }
        if scan.cursor == total {
            players::tell(
                &scan.player,
                &format!(
                    "Scan finished: {} kinetic blocks; {}/{} tracked",
                    scan.found,
                    self.records.len(),
                    MAX_NODES
                ),
            );
        } else {
            self.scans.push_back(scan);
        }
    }
    fn describe(&self, dim: &str, pos: Pos) -> Result<String, String> {
        let node = self
            .live
            .get(dim)
            .and_then(|m| m.get(&pos))
            .ok_or("No loaded kinetic block there. Place it or /kinetics scan")?;
        let sol = &self.solutions[dim];
        let net = &sol.networks[sol.network_of[&pos]];
        Ok(format!(
            "{} [{} axis] at {} {} {}: {} RPM; {:?}; stress {:.0}/{:.0} SU; {} blocks",
            node.kind.id(),
            node.axis.name(),
            pos[0],
            pos[1],
            pos[2],
            sol.speeds[&pos],
            net.state,
            net.stress,
            net.capacity,
            net.members.len()
        ))
    }
    fn save(&mut self) -> Result<(), String> {
        let Some(slot) = &self.slot else {
            self.dirty_save = false;
            return Ok(());
        };
        let mut out = String::from("CHNET2\n");
        for ((dim, pos), r) in &self.records {
            out.push_str(&format!(
                "{dim} {} {} {} {} {} {}\n",
                pos[0],
                pos[1],
                pos[2],
                r.kind.id(),
                r.direction,
                u8::from(r.engaged)
            ));
        }
        storage::set(&format!("net-{slot}"), out.as_bytes())?;
        let mut labs = String::from("CHLABS1\n");
        for (id, lab) in &self.labs {
            let text = lab.machine.encode();
            labs.push_str(&format!(
                "{id} {} {} {} {} {}\n{text}",
                lab.at.0,
                lab.at.1[0],
                lab.at.1[1],
                lab.at.1[2],
                text.len()
            ));
        }
        storage::set(&format!("lab-{slot}"), labs.as_bytes())?;
        self.dirty_save = false;
        Ok(())
    }
    fn select_slot(&mut self, name: &str) -> Result<String, String> {
        if name.is_empty()
            || name.len() > 24
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err("Use a slot name of 1–24 lowercase letters, numbers or underscores".into());
        }
        if self.slot.as_deref() == Some(name) {
            return Ok(format!("World slot {name} is already selected"));
        }
        let mut records = BTreeMap::new();
        if let Some(bytes) = storage::get(&format!("net-{name}")) {
            let text = std::str::from_utf8(&bytes).map_err(|_| "Save is not UTF-8")?;
            let mut lines = text.lines();
            if lines.next() != Some("CHNET2") {
                return Err("Unknown network save version".into());
            }
            for line in lines {
                let f: Vec<_> = line.split_whitespace().collect();
                let [dim, x, y, z, kind, dir, engaged] = f.as_slice() else {
                    return Err("Malformed saved network row".into());
                };
                if !create_simulation::valid_id(dim) {
                    return Err("Malformed saved dimension".into());
                }
                let pos = [
                    x.parse().map_err(|_| "Invalid saved x")?,
                    y.parse().map_err(|_| "Invalid saved y")?,
                    z.parse().map_err(|_| "Invalid saved z")?,
                ];
                let kind = Kind::from_id(kind).ok_or("Unknown saved kinetic block")?;
                let direction = match *dir {
                    "1" => 1,
                    "-1" => -1,
                    _ => return Err("Invalid saved direction".into()),
                };
                let engaged = match *engaged {
                    "0" => false,
                    "1" => true,
                    _ => return Err("Invalid saved clutch".into()),
                };
                if records
                    .insert(
                        ((*dim).into(), pos),
                        Record {
                            kind,
                            direction,
                            engaged,
                        },
                    )
                    .is_some()
                {
                    return Err("Duplicate saved block".into());
                }
                if records.len() > MAX_NODES {
                    return Err("Save exceeds the kinetic block limit".into());
                }
            }
        }
        let mut labs = BTreeMap::new();
        if let Some(bytes) = storage::get(&format!("lab-{name}")) {
            let text = std::str::from_utf8(&bytes).map_err(|_| "Laboratory save is not UTF-8")?;
            let mut rest = text
                .strip_prefix("CHLABS1\n")
                .ok_or("Unknown laboratory save version")?;
            while !rest.is_empty() {
                let (header, tail) = rest.split_once('\n').ok_or("Malformed laboratory header")?;
                let fields: Vec<_> = header.split_whitespace().collect();
                let [id, dim, x, y, z, n] = fields.as_slice() else {
                    return Err("Malformed laboratory header".into());
                };
                let n: usize = n.parse().map_err(|_| "Invalid laboratory save size")?;
                let machine = Machine::decode(tail.get(..n).ok_or("Truncated laboratory save")?)?;
                let pos = [
                    x.parse().map_err(|_| "Invalid lab x")?,
                    y.parse().map_err(|_| "Invalid lab y")?,
                    z.parse().map_err(|_| "Invalid lab z")?,
                ];
                labs.insert(
                    (*id).into(),
                    Lab {
                        at: ((*dim).into(), pos),
                        machine,
                        last: TickResult::Idle,
                    },
                );
                if labs.len() > MAX_LABS {
                    return Err("Too many saved laboratories".into());
                }
                rest = tail.get(n..).ok_or("Malformed laboratory save boundary")?;
            }
        }
        self.save()?;
        self.slot = Some(name.into());
        self.records = records;
        self.labs = labs;
        self.live.clear();
        self.solutions.clear();
        self.cranks.clear();
        self.scans.clear();
        self.dirty_save = false;
        self.refresh();
        Ok(format!(
            "Selected world slot {name}; {} remembered blocks. Use this name only for this world.",
            self.records.len()
        ))
    }
    fn laboratory(&mut self, player: &Player, args: &str) -> Result<String, String> {
        creative(player)?;
        let f: Vec<_> = args.split_whitespace().collect();
        if f.is_empty() || f[0] == "help" {
            return Ok("Virtual recipe lab: /create_lab open <station> <x> <y> <z>; put <id> <count>; fluid <id> <mB>; heat none|heated|superheated; show; clear; recipes [filter]. No real items are consumed or given.".into());
        }
        if f[0] == "recipes" {
            let filter = f.get(1).copied().unwrap_or("");
            let all: Vec<_> = catalog::recipes()
                .iter()
                .filter(|r| r.id.contains(filter) || r.station.contains(filter))
                .collect();
            let ids = all
                .iter()
                .take(6)
                .map(|r| r.id)
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(format!("{} lab recipes match; first six: {ids}", all.len()));
        }
        if let ["open", station, x, y, z] = f.as_slice() {
            let pos = [
                x.parse().map_err(|_| "Use integer x")?,
                y.parse().map_err(|_| "Use integer y")?,
                z.parse().map_err(|_| "Use integer z")?,
            ];
            self.learn(&player.dimension, pos);
            self.refresh();
            let node = self
                .live
                .get(&player.dimension)
                .and_then(|m| m.get(&pos))
                .ok_or("No loaded kinetic machine there")?;
            if !station_matches(station, node.kind) {
                return Err("Station does not match this machine. Use milling, pressing, compacting, mixing, cutting, crushing, deploying, splashing or haunting".into());
            }
            if self.labs.contains_key(&player.id) {
                return Err(
                    "A lab is already open. Use /create_lab clear before replacing it".into(),
                );
            }
            if self.labs.len() >= MAX_LABS {
                return Err("Laboratory limit reached".into());
            }
            self.labs.insert(
                player.id.clone(),
                Lab {
                    at: (player.dimension.clone(), pos),
                    machine: Machine::new(station),
                    last: TickResult::Idle,
                },
            );
            self.dirty_save = true;
            return Ok(format!(
                "Opened virtual {station} lab, driven by the placed machine's RPM. No real item inventory is connected."
            ));
        }
        if f.as_slice() == ["clear"] {
            self.labs.remove(&player.id);
            self.dirty_save = true;
            return Ok("Virtual laboratory cleared".into());
        }
        let lab = self
            .labs
            .get_mut(&player.id)
            .ok_or("Open a lab first: /create_lab open <station> <x> <y> <z>")?;
        match f.as_slice() {
            ["put" | "fluid", id, n] => {
                let amount: u32 = n.parse().map_err(|_| "Use a positive whole amount")?;
                lab.machine.input.insert(id, amount, f[0] == "fluid")?;
                self.dirty_save = true;
                Ok(format!("Virtual input: {}", lab.machine.input.summary()))
            }
            ["heat", h] => {
                lab.machine.heat = match *h {
                    "none" => Heat::None,
                    "heated" => Heat::Heated,
                    "superheated" => Heat::Superheated,
                    _ => return Err("Heat must be none, heated or superheated".into()),
                };
                self.dirty_save = true;
                Ok("Laboratory heat updated".into())
            }
            ["show"] => Ok(format!(
                "Virtual {}: {:?}; {:.0} work; input [{}]; output [{}]",
                lab.machine.station,
                lab.last,
                lab.machine.progress,
                lab.machine.input.summary(),
                lab.machine.output.summary()
            )),
            _ => Err("Unknown laboratory command. Use /create_lab help".into()),
        }
    }
}
impl ServerMod for CreateMod {
    fn init() -> Result<(Self, Registration), String> {
        Ok((
            Self {
                registry: Registry::new(),
                records: BTreeMap::new(),
                live: BTreeMap::new(),
                solutions: BTreeMap::new(),
                cranks: BTreeMap::new(),
                scans: VecDeque::new(),
                labs: BTreeMap::new(),
                slot: None,
                tick: 0,
                dirty_save: false,
            },
            Registration::new()
                .command_with(
                    "kinetics",
                    [
                        Argument::word("action"),
                        Argument::block_pos("pos").optional(),
                        Argument::integer("radius", Some(1), Some(MAX_SCAN)).optional(),
                    ],
                )
                .command("create_lab")
                .command_with("create_save", [])
                .command_with("create_slot", [Argument::word("name")])
                .on_tick()
                .on_blocks(),
        ))
    }
    fn tick(&mut self, tick: u64) {
        self.tick = tick;
        self.cranks.retain(|_, end| *end > tick);
        self.scan_tick();
        self.refresh();
        for lab in self.labs.values_mut() {
            let compatible = self
                .live
                .get(&lab.at.0)
                .and_then(|m| m.get(&lab.at.1))
                .is_some_and(|n| station_matches(&lab.machine.station, n.kind));
            let rpm = if compatible {
                self.solutions
                    .get(&lab.at.0)
                    .and_then(|s| s.speeds.get(&lab.at.1))
                    .copied()
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let old_progress = lab.machine.progress;
            lab.last = lab.machine.tick(rpm);
            if lab.machine.progress != old_progress || lab.last == TickResult::Completed {
                self.dirty_save = true;
            }
        }
        if self.dirty_save
            && tick.is_multiple_of(20)
            && let Err(e) = self.save()
        {
            horizon_mod_sdk::warn(&format!("Save failed; will retry: {e}"));
        }
    }
    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        self.record(&player.dimension, p(pos), state);
    }
    fn block_broken(&mut self, player: &Player, pos: BlockPos, _state: &str) {
        let key = (player.dimension.clone(), p(pos));
        if self.records.remove(&key).is_some() {
            self.dirty_save = true;
        }
        self.cranks.remove(&key);
    }
    fn command(
        &mut self,
        player: &Player,
        name: &str,
        args: &Args,
    ) -> Result<Option<String>, String> {
        if name == "create_save" {
            creative(player)?;
            if self.slot.is_none() {
                return Err("Choose a world slot first: /create_slot <name>".into());
            }
            self.save()?;
            return Ok(Some(
                "Saved this slot's networks and virtual laboratories".into(),
            ));
        }
        if name == "create_lab" {
            return self.laboratory(player, args.text()).map(Some);
        }
        if name == "create_slot" {
            creative(player)?;
            return self
                .select_slot(args.string("name").unwrap_or(""))
                .map(Some);
        }
        let action = args.string("action").unwrap_or("");
        let at = args.block_pos("pos").map(p).unwrap_or_else(|| feet(player));
        if action == "scan" {
            if self.scans.len() >= 4 || self.scans.iter().any(|s| s.player == player.id) {
                return Err("A scan is already queued; wait for it to finish".into());
            }
            let radius = args.integer("radius").unwrap_or(5);
            self.scans.push_back(Scan {
                dimension: player.dimension.clone(),
                centre: at,
                radius,
                cursor: 0,
                found: 0,
                player: player.id.clone(),
            });
            return Ok(Some(format!(
                "Queued radius-{radius} scan; {SCAN_BUDGET} blocks per tick"
            )));
        }
        if action == "list" {
            self.refresh();
            let lines = self
                .solutions
                .get(&player.dimension)
                .map(|s| {
                    s.networks
                        .iter()
                        .take(8)
                        .map(|n| {
                            format!(
                                "{:?}: {} blocks, {:.0}/{:.0} SU",
                                n.state,
                                n.members.len(),
                                n.stress,
                                n.capacity
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            return Ok(Some(if lines.is_empty() {
                "No loaded kinetic networks".into()
            } else {
                lines
            }));
        }
        if action == "info" {
            self.learn(&player.dimension, at);
            self.refresh();
            return self.describe(&player.dimension, at).map(Some);
        }
        if !matches!(action, "flip" | "toggle" | "crank") {
            return Err("/kinetics info|scan|list|flip|toggle|crank [x y z] [radius]".into());
        }
        creative(player)?;
        self.learn(&player.dimension, at);
        self.refresh();
        if self
            .live
            .get(&player.dimension)
            .and_then(|m| m.get(&at))
            .is_none()
        {
            return Err("No loaded kinetic block there".into());
        }
        let key = (player.dimension.clone(), at);
        let record = self.records.get_mut(&key).unwrap();
        let message = match action {
            "flip" if record.kind.is_source() || record.kind == Kind::Gearshift => {
                record.direction = -record.direction;
                "Reversed source or gearshift"
            }
            "toggle" if record.kind == Kind::Clutch => {
                record.engaged = !record.engaged;
                "Toggled clutch"
            }
            "crank" if record.kind == Kind::HandCrank => {
                self.cranks.insert(key, self.tick.saturating_add(200));
                "Cranking for 10 seconds"
            }
            _ => return Err(
                "flip needs a source/gearshift; toggle needs a clutch; crank needs a hand crank"
                    .into(),
            ),
        };
        self.dirty_save = true;
        self.refresh();
        self.save()?;
        Ok(Some(message.into()))
    }
}
horizon_mod_sdk::export_server_mod!(CreateMod);
