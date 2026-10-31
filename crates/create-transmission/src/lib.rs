//! Rotation propagation and stress networks, after Create 6.0.10.
//!
//! The rules and numbers here were read from the 1.21.1 jar
//! (`RotationPropagator`, `ICogWheel`, `AllBlocks`, `CKinetics`):
//!
//! * a shaft joins the blocks at both ends of its axis, speed unchanged;
//! * two small cogwheels side by side (same axis, offset along another axis)
//!   turn against each other at the same speed (modifier -1);
//! * a large cogwheel and a small one meet diagonally in the plane of their
//!   axis: large to small is -2, small to large is -0.5;
//! * a network's capacity is the sum of `capacity * |speed|` of its sources,
//!   its stress the sum of `impact * |speed|` of every block, and it is
//!   overstressed, and stands still, when stress is greater than capacity;
//! * no block turns faster than 256 RPM (`CKinetics.maxRotationSpeed`).
//!
//! Belt transport, windmill sails, and contraptions remain separate work.
//! The host supplies redstone, crank and water source state.
//!
//! The crate has no dependencies and knows nothing about Horizon: the mod
//! feeds it the blocks it sees and reads speeds back.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// A block position: x, y, z.
pub type Pos = [i32; 3];

/// `CKinetics.maxRotationSpeed`.
pub const MAX_RPM: f32 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parse(s: &str) -> Option<Axis> {
        match s {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        ["x", "y", "z"][self.index()]
    }
}

/// What a block does in a network. Numbers are Create's defaults (stress
/// units per RPM, and the RPM a source turns at).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Shaft,
    Cogwheel,
    LargeCogwheel,
    CreativeMotor,
    HandCrank,
    WaterWheel,
    LargeWaterWheel,
    EncasedFan,
    Millstone,
    MechanicalPress,
    MechanicalMixer,
    MechanicalCrafter,
    MechanicalSaw,
    MechanicalDrill,
    Clutch,
    Gearshift,
    CrushingWheel,
    Deployer,
    Gearbox,
    EncasedChainDrive,
    Belt,
    MechanicalPump,
}

impl Kind {
    pub const ALL: [Kind; 22] = [
        Kind::Shaft,
        Kind::Cogwheel,
        Kind::LargeCogwheel,
        Kind::CreativeMotor,
        Kind::HandCrank,
        Kind::WaterWheel,
        Kind::LargeWaterWheel,
        Kind::EncasedFan,
        Kind::Millstone,
        Kind::MechanicalPress,
        Kind::MechanicalMixer,
        Kind::MechanicalCrafter,
        Kind::MechanicalSaw,
        Kind::MechanicalDrill,
        Kind::Clutch,
        Kind::Gearshift,
        Kind::CrushingWheel,
        Kind::Deployer,
        Kind::Gearbox,
        Kind::EncasedChainDrive,
        Kind::Belt,
        Kind::MechanicalPump,
    ];

    /// The block's id path in the `create` namespace.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Shaft => "shaft",
            Kind::Cogwheel => "cogwheel",
            Kind::LargeCogwheel => "large_cogwheel",
            Kind::CreativeMotor => "creative_motor",
            Kind::HandCrank => "hand_crank",
            Kind::WaterWheel => "water_wheel",
            Kind::LargeWaterWheel => "large_water_wheel",
            Kind::EncasedFan => "encased_fan",
            Kind::Millstone => "millstone",
            Kind::MechanicalPress => "mechanical_press",
            Kind::MechanicalMixer => "mechanical_mixer",
            Kind::MechanicalCrafter => "mechanical_crafter",
            Kind::MechanicalSaw => "mechanical_saw",
            Kind::MechanicalDrill => "mechanical_drill",
            Kind::Clutch => "clutch",
            Kind::Gearshift => "gearshift",
            Kind::CrushingWheel => "crushing_wheel",
            Kind::Deployer => "deployer",
            Kind::Gearbox => "gearbox",
            Kind::EncasedChainDrive => "encased_chain_drive",
            Kind::Belt => "belt",
            Kind::MechanicalPump => "mechanical_pump",
        }
    }

    pub fn from_id(path: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == path)
    }

    /// Stress units per RPM this block takes from its network.
    pub fn impact(self) -> f32 {
        match self {
            Kind::EncasedFan => 2.0,
            Kind::Millstone => 4.0,
            Kind::MechanicalPress => 8.0,
            Kind::MechanicalMixer => 4.0,
            Kind::MechanicalCrafter => 2.0,
            Kind::MechanicalSaw => 4.0,
            Kind::MechanicalDrill => 4.0,
            Kind::CrushingWheel => 8.0,
            Kind::Deployer | Kind::MechanicalPump => 4.0,
            Kind::Belt => 1.0,
            _ => 0.0,
        }
    }

    /// Stress units per RPM this block gives its network, at its own speed.
    pub fn capacity(self) -> f32 {
        match self {
            Kind::CreativeMotor => 16384.0,
            Kind::HandCrank => 8.0,
            Kind::WaterWheel => 32.0,
            Kind::LargeWaterWheel => 128.0,
            _ => 0.0,
        }
    }

    /// The RPM a source turns at, before its direction; 0 for the rest.
    pub fn generated_rpm(self) -> f32 {
        match self {
            Kind::CreativeMotor => 256.0,
            Kind::HandCrank => 32.0,
            Kind::WaterWheel => 8.0,
            Kind::LargeWaterWheel => 4.0,
            _ => 0.0,
        }
    }

    pub fn is_source(self) -> bool {
        self.capacity() > 0.0
    }

    pub fn is_small_cog(self) -> bool {
        matches!(
            self,
            Kind::Cogwheel | Kind::Millstone | Kind::MechanicalMixer | Kind::MechanicalPump
        )
    }

    pub fn is_large_cog(self) -> bool {
        self == Kind::LargeCogwheel
    }
}

/// One block of a network.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub kind: Kind,
    pub axis: Axis,
    /// +1 or -1: which way a source turns. Ignored by the rest.
    pub direction: i8,
    /// A source that is switched off (a wheel with no water, a crank nobody
    /// turns) generates nothing.
    pub active: bool,
    /// Six shaft ports: -X,+X,-Y,+Y,-Z,+Z. A node's RPM is measured at
    /// its negative-axis half. A powered gearshift reverses its positive half.
    pub ports: u8,
    /// Optional configured source speed, clamped to the network limit.
    pub configured_rpm: Option<f32>,
}

impl Node {
    pub fn new(kind: Kind, axis: Axis) -> Node {
        Node {
            kind,
            axis,
            direction: 1,
            configured_rpm: None,
            active: true,
            ports: if kind == Kind::Gearbox {
                0b111111 ^ (3 << (axis.index() * 2))
            } else {
                3 << (axis.index() * 2)
            },
        }
    }

    fn generated(&self) -> f32 {
        if self.kind.is_source() && self.active {
            (match self.configured_rpm {
                Some(r) if r.is_finite() => r.clamp(0.0, MAX_RPM),
                Some(_) => 0.0,
                None => self.kind.generated_rpm().min(MAX_RPM),
            }) * f32::from(self.direction.signum())
        } else {
            0.0
        }
    }
}

/// What a network is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No source is on.
    Idle,
    Running,
    /// Stress is greater than capacity: everything in it stands still.
    Overstressed,
    /// Two sources push against each other, or a block would pass 256 RPM.
    Conflict,
}

#[derive(Clone, Debug)]
pub struct Network {
    /// Every block of the network, sorted.
    pub members: Vec<Pos>,
    pub state: State,
    /// Stress units the sources give, and the blocks take, at the speeds the
    /// network would turn at; both 0 when nothing would turn.
    pub capacity: f32,
    pub stress: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Solution {
    /// The RPM of each block, signed: 0 for a block that stands still.
    pub speeds: HashMap<Pos, f32>,
    /// Index into `networks` for each block.
    pub network_of: HashMap<Pos, usize>,
    pub networks: Vec<Network>,
}

fn manhattan(d: Pos) -> i32 {
    d.iter().map(|v| v.abs()).sum()
}

/// `IRotate.hasShaftTowards`: every block here has a shaft at both ends of
/// its axis.
fn has_shaft_towards(n: &Node, d: Pos) -> bool {
    if n.kind == Kind::Clutch && !n.active {
        return false;
    }
    let Some(i) = d.iter().position(|v| *v != 0) else {
        return false;
    };
    n.ports & (1 << (i * 2 + usize::from(d[i] > 0))) != 0
}

fn half_sign(n: &Node, d: Pos) -> f32 {
    if n.kind == Kind::Gearbox {
        let port = d.iter().position(|v| *v != 0).unwrap();
        let first = (0..3).find(|i| *i != n.axis.index()).unwrap();
        return if port == first {
            1.0
        } else {
            -d[port].signum() as f32
        };
    }
    if n.kind == Kind::Belt {
        return f32::from(n.direction.signum());
    }
    if n.kind == Kind::Gearshift && d[n.axis.index()] > 0 && n.direction < 0 {
        -1.0
    } else {
        1.0
    }
}

/// `RotationPropagator.getRotationSpeedModifier`, from `a` to `b`, where
/// `d` is `b - a`. None when they are not connected.
pub fn modifier(a: &Node, b: &Node, d: Pos) -> Option<f32> {
    if d.iter().any(|v| !(-1..=1).contains(v)) {
        return None;
    }
    // The axis along which the two sit, when they touch face to face.
    let touching = (manhattan(d) == 1).then(|| d.iter().position(|v| *v != 0).unwrap());

    if a.kind == Kind::MechanicalCrafter
        && b.kind == Kind::MechanicalCrafter
        && a.axis == b.axis
        && touching.is_some()
        && d[a.axis.index()] == 0
    {
        return Some(-1.0);
    }

    // A shaft runs through both: same axis, along it.
    if touching.is_some() && has_shaft_towards(a, d) && has_shaft_towards(b, d.map(|v| -v)) {
        return Some(half_sign(a, d) / half_sign(b, d.map(|v| -v)));
    }

    if a.kind == Kind::Belt
        && b.kind == Kind::Belt
        && a.axis == b.axis
        && manhattan(d) == 1
        && d[1] == 0
        && d[a.axis.index()] == 0
    {
        return Some(f32::from(a.direction.signum()) / f32::from(b.direction.signum()));
    }

    if a.kind == Kind::EncasedChainDrive
        && b.kind == Kind::EncasedChainDrive
        && a.axis == b.axis
        && manhattan(d) == 1
        && d[a.axis.index()] == 0
    {
        return Some(1.0);
    }

    // Perpendicular large cogs meet on the diagonal of their two axes.
    // RotationPropagator.isLargeToLargeGear, not parallel large-cog teeth.
    if a.kind.is_large_cog() && b.kind.is_large_cog() && a.axis != b.axis {
        let ai = a.axis.index();
        let bi = b.axis.index();
        if d[ai].abs() == 1
            && d[bi].abs() == 1
            && d.iter()
                .enumerate()
                .all(|(i, v)| i == ai || i == bi || *v == 0)
        {
            return Some(if (d[ai] > 0) ^ (d[bi] > 0) { -1.0 } else { 1.0 });
        }
    }

    // Large and small cogwheels: same axis, diagonal in the plane around it.
    let same_axis = a.axis == b.axis;
    let in_plane_diagonal = d[a.axis.index()] == 0
        && d.iter()
            .enumerate()
            .filter(|(i, _)| *i != a.axis.index())
            .all(|(_, v)| v.abs() == 1);
    if same_axis && in_plane_diagonal {
        if a.kind.is_large_cog() && b.kind.is_small_cog() {
            return Some(-2.0);
        }
        if a.kind.is_small_cog() && b.kind.is_large_cog() {
            return Some(-0.5);
        }
    }

    // Two small cogwheels side by side, never along their axis.
    if a.kind.is_small_cog()
        && b.kind.is_small_cog()
        && same_axis
        && let Some(i) = touching
        && i != a.axis.index()
    {
        return Some(-1.0);
    }

    None
}

const NEIGHBOURS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// Where a block may connect to: its six faces, and the eight diagonals in
/// the plane of a cogwheel's axis.
fn candidate_offsets(n: &Node) -> Vec<Pos> {
    let mut v: Vec<Pos> = NEIGHBOURS.to_vec();
    if n.kind.is_large_cog() || n.kind.is_small_cog() {
        let others: Vec<usize> = (0..3).filter(|i| *i != n.axis.index()).collect();
        for s0 in [-1, 1] {
            for s1 in [-1, 1] {
                let mut d = [0; 3];
                d[others[0]] = s0;
                d[others[1]] = s1;
                v.push(d);
            }
        }
    }
    if n.kind.is_large_cog() {
        for other in 0..3 {
            if other == n.axis.index() {
                continue;
            }
            for sa in [-1, 1] {
                for sb in [-1, 1] {
                    let mut d = [0; 3];
                    d[n.axis.index()] = sa;
                    d[other] = sb;
                    v.push(d);
                }
            }
        }
    }
    v
}

fn offset(p: Pos, d: Pos) -> Option<Pos> {
    Some([
        p[0].checked_add(d[0])?,
        p[1].checked_add(d[1])?,
        p[2].checked_add(d[2])?,
    ])
}

/// Works out every block's speed and every network's stress.
pub fn solve(nodes: &BTreeMap<Pos, Node>) -> Solution {
    let mut sol = Solution::default();
    let mut seen: BTreeSet<Pos> = BTreeSet::new();

    for &start in nodes.keys() {
        if seen.contains(&start) {
            continue;
        }
        // The connected component, with each edge's modifier. Edges are kept
        // only when connected both ways.
        let mut members: Vec<Pos> = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen.insert(start);
        while let Some(p) = queue.pop_front() {
            members.push(p);
            for d in candidate_offsets(&nodes[&p]) {
                let Some(q) = offset(p, d) else { continue };
                let Some(nb) = nodes.get(&q) else { continue };
                if modifier(&nodes[&p], nb, d).is_some() && !seen.contains(&q) {
                    seen.insert(q);
                    queue.push_back(q);
                }
            }
        }
        members.sort();
        let idx = sol.networks.len();
        let report = solve_component(nodes, &members, &mut sol.speeds);
        for p in &members {
            sol.network_of.insert(*p, idx);
        }
        sol.networks.push(report);
    }
    sol
}

fn solve_component(
    nodes: &BTreeMap<Pos, Node>,
    members: &[Pos],
    speeds: &mut HashMap<Pos, f32>,
) -> Network {
    let still = |speeds: &mut HashMap<Pos, f32>| {
        for p in members {
            speeds.insert(*p, 0.0);
        }
    };

    // Solve relative ratios before choosing a driver. Comparing raw source
    // RPM is wrong when a slower source drives through a speed-up gear.
    let root = members[0];
    // 512 blocks can exceed f32's exponent range before reaching a driver.
    let mut ratios = HashMap::from([(root, 1.0_f64)]);
    let mut queue = VecDeque::from([root]);
    let mut conflict = false;
    while let Some(p) = queue.pop_front() {
        for d in candidate_offsets(&nodes[&p]) {
            let Some(q) = offset(p, d) else { continue };
            let Some(nb) = nodes.get(&q) else { continue };
            let Some(m) = modifier(&nodes[&p], nb, d) else {
                continue;
            };
            let next = ratios[&p] * f64::from(m);
            if !next.is_finite() || next == 0.0 {
                conflict = true;
                continue;
            }
            match ratios.get(&q) {
                None => {
                    ratios.insert(q, next);
                    queue.push_back(q);
                }
                Some(have) if (have - next).abs() > 1e-9 * have.abs().max(next.abs()) => {
                    conflict = true
                }
                _ => {}
            }
        }
    }
    let lead = members
        .iter()
        .filter_map(|p| {
            let g = nodes[p].generated();
            let ratio = ratios.get(p).copied()?;
            (g != 0.0).then_some((*p, f64::from(g) / ratio))
        })
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()).then(b.0.cmp(&a.0)));
    let Some((_, reference)) = lead else {
        still(speeds);
        return Network {
            members: members.to_vec(),
            state: State::Idle,
            capacity: 0.0,
            stress: 0.0,
        };
    };
    let turn: HashMap<Pos, f32> = members
        .iter()
        .map(|p| {
            (
                *p,
                (ratios.get(p).copied().unwrap_or(0.0) * reference) as f32,
            )
        })
        .collect();
    if turn
        .values()
        .any(|s| !s.is_finite() || s.abs() > MAX_RPM + 1e-3)
    {
        conflict = true;
    }
    let mut capacity = 0.0;
    for p in members {
        let n = &nodes[p];
        let g = n.generated();
        if g == 0.0 {
            continue;
        }
        let at = turn[p];
        if (at - g).abs() <= 1e-3 {
            capacity += n.kind.capacity() * g.abs();
        } else if at * g < 0.0 {
            conflict = true;
        }
    }

    let stress: f32 = members
        .iter()
        .map(|p| nodes[p].kind.impact() * turn.get(p).copied().unwrap_or(0.0).abs())
        .sum();

    if conflict {
        still(speeds);
        return Network {
            members: members.to_vec(),
            state: State::Conflict,
            capacity,
            stress,
        };
    }
    if stress > capacity {
        still(speeds);
        return Network {
            members: members.to_vec(),
            state: State::Overstressed,
            capacity,
            stress,
        };
    }
    for p in members {
        speeds.insert(*p, turn.get(p).copied().unwrap_or(0.0));
    }
    Network {
        members: members.to_vec(),
        state: State::Running,
        capacity,
        stress,
    }
}

pub mod state;

#[cfg(test)]
mod tests;
