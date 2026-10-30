#[allow(dead_code)]
mod kinetic {
// Engine-independent prototype. No Horizon runtime adapter exists yet.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    East,
    West,
    Up,
    Down,
    South,
    North,
}
impl Face {
    pub fn axis(self) -> Axis {
        match self {
            Self::East | Self::West => Axis::X,
            Self::Up | Self::Down => Axis::Y,
            _ => Axis::Z,
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::West => Self::East,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::South => Self::North,
            Self::North => Self::South,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
            Self::East => Self::Up,
            Self::Up => Self::Down,
            Self::Down => Self::South,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Placement {
    Axial,
    HorizontalAxial,
    Directional,
    Upright,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Axis(Axis),
    Facing(Face),
}
/// Written placement policy, pending in-game parity validation against Create.
/// Normal axial placement aligns to an existing drive when available; sneaking
/// bypasses alignment. Directional machines face the player in all six directions.
pub fn placement(
    kind: Placement,
    clicked: Face,
    look: Face,
    horizontal: Face,
    sneaking: bool,
    neighbour: Option<Axis>,
) -> Orientation {
    match kind {
        Placement::Axial => Orientation::Axis(if sneaking {
            clicked.axis()
        } else {
            neighbour.unwrap_or(clicked.axis())
        }),
        Placement::HorizontalAxial => Orientation::Axis(
            match if sneaking {
                clicked.axis()
            } else {
                neighbour.unwrap_or(clicked.axis())
            } {
                Axis::Y => horizontal.axis(),
                a => a,
            },
        ),
        Placement::Directional => {
            Orientation::Facing(if sneaking { look } else { look.opposite() })
        }
        Placement::Upright => Orientation::Facing(horizontal.opposite()),
    }
}
pub fn wrench(orientation: Orientation, reverse: bool) -> Orientation {
    match orientation {
        Orientation::Axis(a) => Orientation::Axis(match (a, reverse) {
            (Axis::X, false) | (Axis::Z, true) => Axis::Y,
            (Axis::Y, false) | (Axis::X, true) => Axis::Z,
            _ => Axis::X,
        }),
        Orientation::Facing(mut f) => {
            for _ in 0..if reverse { 5 } else { 1 } {
                f = f.next()
            }
            Orientation::Facing(f)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos(pub i32, pub i32, pub i32);
impl Pos {
    fn along(self, axis: Axis, sign: i32) -> Self {
        match axis {
            Axis::X => Self(self.0 + sign, self.1, self.2),
            Axis::Y => Self(self.0, self.1 + sign, self.2),
            Axis::Z => Self(self.0, self.1, self.2 + sign),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Shaft,
    WaterWheel,
    CreativeMotor(f64),
}
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub axis: Axis,
    pub kind: Kind,
    pub rpm: f64,
    pub angle: f64,
    pub jammed: bool,
}
impl Node {
    pub fn new(axis: Axis, kind: Kind) -> Self {
        Self {
            axis,
            kind,
            rpm: 0.0,
            angle: 0.0,
            jammed: false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FlowSample {
    pub offset: [f64; 3],
    pub velocity: [f64; 3],
}
/// Tangential flow torque around the wheel axis. Still water has no torque.
/// This is a prototype physical rule, not an assertion of original Create parity.
pub fn wheel_rpm(axis: Axis, flow: &[FlowSample]) -> f64 {
    let a = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    let torque: f64 = flow
        .iter()
        .map(|s| {
            let r = s.offset;
            let v = s.velocity;
            [
                r[1] * v[2] - r[2] * v[1],
                r[2] * v[0] - r[0] * v[2],
                r[0] * v[1] - r[1] * v[0],
            ][a]
        })
        .sum();
    if !torque.is_finite() || torque.abs() < 1e-6 {
        0.0
    } else {
        torque.signum() * 8.0
    }
}
#[derive(Default)]
pub struct Network {
    pub nodes: BTreeMap<Pos, Node>,
}
impl Network {
    /// Recompute from topology every tick: removing a source or connection stops
    /// disconnected shafts immediately. Only coaxial, directly touching shafts
    /// and sources connect in this first stage. Conflicting drives jam safely.
    pub fn tick(&mut self, seconds: f64, flows: &BTreeMap<Pos, Vec<FlowSample>>) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let mut remaining: std::collections::BTreeSet<_> = self.nodes.keys().copied().collect();
        for node in self.nodes.values_mut() {
            node.rpm = 0.0;
            node.jammed = false;
        }
        while let Some(start) = remaining.pop_first() {
            let mut queue = VecDeque::from([start]);
            let mut group = vec![];
            let mut drive: Option<f64> = None;
            let mut jam = false;
            while let Some(pos) = queue.pop_front() {
                let node = self.nodes[&pos];
                group.push(pos);
                let rpm = match node.kind {
                    Kind::Shaft => 0.0,
                    Kind::CreativeMotor(r) => {
                        if r.is_finite() {
                            r.clamp(-256.0, 256.0)
                        } else {
                            0.0
                        }
                    }
                    Kind::WaterWheel => {
                        wheel_rpm(node.axis, flows.get(&pos).map(Vec::as_slice).unwrap_or(&[]))
                    }
                };
                if rpm != 0.0 {
                    if let Some(old) = drive {
                        if (old - rpm).abs() > 1e-6 {
                            jam = true
                        }
                    } else {
                        drive = Some(rpm)
                    }
                }
                for sign in [-1, 1] {
                    let next = pos.along(node.axis, sign);
                    if self.nodes.get(&next).is_some_and(|n| n.axis == node.axis)
                        && remaining.remove(&next)
                    {
                        queue.push_back(next)
                    }
                }
            }
            for pos in group {
                let n = self.nodes.get_mut(&pos).unwrap();
                n.jammed = jam;
                n.rpm = if jam { 0.0 } else { drive.unwrap_or(0.0) };
                n.angle = (n.angle + n.rpm * 6.0 * seconds).rem_euclid(360.0);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directional_placement_has_all_six_faces() {
        for f in [
            Face::Up,
            Face::Down,
            Face::East,
            Face::West,
            Face::North,
            Face::South,
        ] {
            assert_eq!(
                placement(
                    Placement::Directional,
                    Face::Up,
                    f,
                    Face::South,
                    false,
                    None
                ),
                Orientation::Facing(f.opposite())
            );
        }
    }
    #[test]
    fn shaft_alignment_can_be_bypassed() {
        assert_eq!(
            placement(
                Placement::Axial,
                Face::Up,
                Face::North,
                Face::North,
                false,
                Some(Axis::X)
            ),
            Orientation::Axis(Axis::X)
        );
        assert_eq!(
            placement(
                Placement::Axial,
                Face::Up,
                Face::North,
                Face::North,
                true,
                Some(Axis::X)
            ),
            Orientation::Axis(Axis::Y)
        );
    }
    #[test]
    fn wheels_stay_horizontal_and_machines_upright() {
        assert_eq!(
            placement(
                Placement::HorizontalAxial,
                Face::Up,
                Face::Down,
                Face::East,
                false,
                None
            ),
            Orientation::Axis(Axis::X)
        );
        assert_eq!(
            placement(
                Placement::Upright,
                Face::Down,
                Face::Up,
                Face::West,
                false,
                None
            ),
            Orientation::Facing(Face::East)
        );
    }
    #[test]
    fn wrench_is_reversible() {
        for o in [Orientation::Axis(Axis::X), Orientation::Facing(Face::Down)] {
            assert_eq!(wrench(wrench(o, false), true), o)
        }
    }
    fn flow(v: f64) -> Vec<FlowSample> {
        vec![FlowSample {
            offset: [1.0, 0.0, 0.0],
            velocity: [0.0, v, 0.0],
        }]
    }
    #[test]
    fn still_water_stops_and_reversed_flow_reverses() {
        assert_eq!(wheel_rpm(Axis::Z, &flow(0.0)), 0.0);
        assert_eq!(wheel_rpm(Axis::Z, &flow(-1.0)), -8.0);
        assert_eq!(wheel_rpm(Axis::Z, &flow(1.0)), 8.0);
    }
    #[test]
    fn radial_flow_does_not_drive_a_wheel() {
        assert_eq!(wheel_rpm(Axis::X, &flow(1.0)), 0.0)
    }
    #[test]
    fn connected_shaft_rotates_then_stops_when_source_removed() {
        let mut n = Network::default();
        n.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::Z, Kind::WaterWheel));
        n.nodes
            .insert(Pos(0, 0, 1), Node::new(Axis::Z, Kind::Shaft));
        let flows = BTreeMap::from([(Pos(0, 0, 0), flow(1.0))]);
        n.tick(1.0, &flows);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 8.0);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].angle, 48.0);
        n.nodes.remove(&Pos(0, 0, 0));
        n.tick(1.0, &flows);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 0.0);
    }
    #[test]
    fn perpendicular_shafts_do_not_connect() {
        let mut n = Network::default();
        n.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::Z, Kind::CreativeMotor(16.0)));
        n.nodes
            .insert(Pos(0, 0, 1), Node::new(Axis::X, Kind::Shaft));
        n.tick(1.0, &BTreeMap::new());
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 0.0);
    }
    #[test]
    fn incompatible_sources_jam_and_recover() {
        let mut n = Network::default();
        for (z, rpm) in [(0, 16.0), (1, -16.0)] {
            n.nodes
                .insert(Pos(0, 0, z), Node::new(Axis::Z, Kind::CreativeMotor(rpm)));
        }
        n.tick(1.0, &BTreeMap::new());
        assert!(n.nodes.values().all(|n| n.jammed && n.rpm == 0.0));
        n.nodes.remove(&Pos(0, 0, 1));
        n.tick(1.0, &BTreeMap::new());
        assert_eq!(n.nodes[&Pos(0, 0, 0)].rpm, 16.0);
    }
    #[test]
    fn rotation_wraps_and_frame_duration_is_consistent() {
        let mut a = Network::default();
        a.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::X, Kind::CreativeMotor(64.0)));
        let mut b = Network {
            nodes: a.nodes.clone(),
        };
        a.tick(1.0, &BTreeMap::new());
        for _ in 0..20 {
            b.tick(0.05, &BTreeMap::new());
        }
        assert!((a.nodes[&Pos(0, 0, 0)].angle - b.nodes[&Pos(0, 0, 0)].angle).abs() < 1e-9);
    }
}

}
#[allow(dead_code)]
pub mod transmission {
// Rotation propagation and stress networks, after Create 6.0.10.
//
// The rules and numbers here were read from the 1.21.1 jar
// (`RotationPropagator`, `ICogWheel`, `AllBlocks`, `CKinetics`):
//
// * a shaft joins the blocks at both ends of its axis, speed unchanged;
// * two small cogwheels side by side (same axis, offset along another axis)
//   turn against each other at the same speed (modifier -1);
// * a large cogwheel and a small one meet diagonally in the plane of their
//   axis: large to small is -2, small to large is -0.5;
// * a network's capacity is the sum of `capacity * |speed|` of its sources,
//   its stress the sum of `impact * |speed|` of every block, and it is
//   overstressed, and stands still, when stress is greater than capacity;
// * no block turns faster than 256 RPM (`CKinetics.maxRotationSpeed`).
//
// Belt transport, windmill sails, and contraptions remain separate work.
// The host supplies redstone, crank and water source state.
//
// The crate has no dependencies and knows nothing about Horizon: the mod
// feeds it the blocks it sees and reads speeds back.

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
    pub const ALL: [Kind; 21] = [
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
            Kind::MechanicalSaw => 4.0,
            Kind::MechanicalDrill => 4.0,
            Kind::CrushingWheel => 8.0,
            Kind::Deployer | Kind::MechanicalPump => 4.0,
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
            Kind::Cogwheel | Kind::Millstone | Kind::MechanicalMixer
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

    // A shaft runs through both: same axis, along it.
    if touching.is_some() && has_shaft_towards(a, d) && has_shaft_towards(b, d.map(|v| -v)) {
        return Some(half_sign(a, d) / half_sign(b, d.map(|v| -v)));
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

pub mod state {
// Reading Horizon's block state strings (`create:shaft[axis=y]`) into
// kinetic nodes.

use super::{Axis, Kind, Node};

/// The `create` namespace's kinetic blocks. Kept as a type so a later
/// version can add other namespaces (an addon's blocks) without changing the
/// callers.
pub struct Registry {
    namespace: &'static str,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            namespace: "create",
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// `create:shaft[axis=y]` as a node; None for any other block. A block with
/// no `axis` turns about Y, as Create's default does.
pub fn parse_state(reg: &Registry, state: &str) -> Option<Node> {
    let (id, props) = match state.split_once('[') {
        Some((id, rest)) => (id, rest.strip_suffix(']')?),
        None => (state, ""),
    };
    let mut properties = std::collections::BTreeMap::new();
    if !props.is_empty() {
        for p in props.split(',') {
            let (k, v) = p.split_once('=')?;
            if k.is_empty() || v.is_empty() || properties.insert(k, v).is_some() {
                return None;
            }
        }
    }
    let path = id.strip_prefix(reg.namespace)?.strip_prefix(':')?;
    let kind = Kind::from_id(path)?;
    let axis = match properties.get("axis") {
        Some(a) => Axis::parse(a)?,
        None => Axis::Y,
    };
    let mut node = Node::new(kind, axis);
    if let Some(facing) = properties.get("facing") {
        let (axis, positive) = match *facing {
            "east" => (Axis::X, true),
            "west" => (Axis::X, false),
            "up" => (Axis::Y, true),
            "down" => (Axis::Y, false),
            "south" => (Axis::Z, true),
            "north" => (Axis::Z, false),
            _ => return None,
        };
        node.axis = axis;
        node.ports = match kind {
            Kind::MechanicalPump => 3 << (axis.index() * 2),
            Kind::CreativeMotor => 1 << (axis.index() * 2 + usize::from(positive)),
            _ => 1 << (axis.index() * 2 + usize::from(!positive)),
        };
        match kind {
            Kind::MechanicalPress => {
                node.axis = if axis == Axis::Y { Axis::Z } else { axis };
                node.ports = 3 << (node.axis.index() * 2);
            }
            Kind::MechanicalSaw => {
                node.axis = if axis == Axis::Y { Axis::Z } else { Axis::Y };
                node.ports = 3 << (node.axis.index() * 2);
            }
            Kind::MechanicalMixer => {
                node.axis = Axis::Y;
                node.ports = 1 << 3;
            }
            Kind::Millstone => {
                node.axis = Axis::Y;
                node.ports = 1 << 2;
            }
            Kind::Deployer => {
                node.axis = if axis == Axis::Y { Axis::X } else { Axis::Y };
                node.ports = 3 << (node.axis.index() * 2);
            }
            _ => {}
        }
    }
    if let Some(powered) = properties.get("powered") {
        if !matches!(*powered, "true" | "false") {
            return None;
        }
        if kind == Kind::Clutch {
            node.active = *powered == "false";
        }
        if kind == Kind::Gearshift {
            node.direction = if *powered == "true" { -1 } else { 1 };
        }
    }
    // Sources that need something outside the block start off: wheels wait
    // for the water check, cranks for a player.
    if matches!(
        kind,
        Kind::WaterWheel | Kind::LargeWaterWheel | Kind::HandCrank
    ) {
        node.active = false;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kind_and_axis() {
        let r = Registry::new();
        let n = parse_state(&r, "create:shaft[axis=x]").unwrap();
        assert_eq!((n.kind, n.axis), (Kind::Shaft, Axis::X));
        let n = parse_state(&r, "create:large_cogwheel[axis=z,waterlogged=false]").unwrap();
        assert_eq!((n.kind, n.axis), (Kind::LargeCogwheel, Axis::Z));
    }

    #[test]
    fn defaults_to_y_and_ignores_other_blocks() {
        let r = Registry::new();
        assert_eq!(parse_state(&r, "create:cogwheel").unwrap().axis, Axis::Y);
        assert!(parse_state(&r, "minecraft:stone").is_none());
        assert!(parse_state(&r, "create:unknown_block").is_none());
        assert!(parse_state(&r, "createx:shaft").is_none());
    }

    #[test]
    fn wheels_and_cranks_start_off() {
        let r = Registry::new();
        assert!(
            !parse_state(&r, "create:water_wheel[axis=x]")
                .unwrap()
                .active
        );
        assert!(!parse_state(&r, "create:hand_crank[axis=x]").unwrap().active);
        assert!(
            parse_state(&r, "create:creative_motor[axis=x]")
                .unwrap()
                .active
        );
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;
    #[test]
    fn physical_output_and_input_ports_follow_facing() {
        let r = Registry::new();
        assert_eq!(
            parse_state(&r, "create:creative_motor[facing=south]")
                .unwrap()
                .ports,
            1 << 5
        );
        assert_eq!(
            parse_state(&r, "create:mechanical_drill[facing=south]")
                .unwrap()
                .ports,
            1 << 4
        );
        assert_eq!(
            parse_state(&r, "create:encased_fan[facing=north]")
                .unwrap()
                .ports,
            1 << 5
        );
    }
    #[test]
    fn mixer_and_millstone_accept_vertical_drive() {
        let r = Registry::new();
        let m = parse_state(&r, "create:mechanical_mixer[facing=north]").unwrap();
        assert_eq!(m.axis, Axis::Y);
        assert_eq!(m.ports, 1 << 3);
        let m = parse_state(&r, "create:millstone[facing=west]").unwrap();
        assert_eq!(m.axis, Axis::Y);
        assert_eq!(m.ports, 1 << 2);
    }
}

}

#[cfg(test)]
mod tests {
use super::*;

fn world(blocks: &[(Pos, Kind, Axis)]) -> BTreeMap<Pos, Node> {
    blocks
        .iter()
        .map(|(p, k, a)| (*p, Node::new(*k, *a)))
        .collect()
}

#[test]
fn ids_round_trip() {
    for k in Kind::ALL {
        assert_eq!(Kind::from_id(k.id()), Some(k));
    }
}

#[test]
fn a_shaft_line_keeps_its_speed() {
    let w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
        ([2, 0, 0], Kind::Shaft, Axis::X),
        ([3, 0, 0], Kind::Millstone, Axis::X),
    ]);
    let s = solve(&w);
    assert_eq!(s.networks.len(), 1);
    assert_eq!(s.networks[0].state, State::Running);
    for x in 0..4 {
        assert_eq!(s.speeds[&[x, 0, 0]], 32.0);
    }
    // Hand crank: 8 SU per RPM at 32 RPM; millstone 4 SU per RPM.
    assert_eq!(s.networks[0].capacity, 256.0);
    assert_eq!(s.networks[0].stress, 128.0);
}

#[test]
fn a_shaft_crossways_does_not_connect() {
    let w = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.networks.len(), 2);
    assert_eq!(s.speeds[&[1, 0, 0]], 0.0);
}

#[test]
fn small_cogs_side_by_side_reverse() {
    let w = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::Y),
        ([1, 0, 0], Kind::Cogwheel, Axis::Y),
        ([1, 1, 0], Kind::Cogwheel, Axis::Y),
    ]);
    // The motor does not mesh with a cog beside it: only along Y.
    let s = solve(&w);
    assert_eq!(s.networks.len(), 2);

    let w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::Y),
        ([0, 1, 0], Kind::Cogwheel, Axis::Y),
        ([1, 1, 0], Kind::Cogwheel, Axis::Y),
        ([2, 1, 0], Kind::Cogwheel, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.speeds[&[0, 1, 0]], 32.0);
    assert_eq!(s.speeds[&[1, 1, 0]], -32.0);
    assert_eq!(s.speeds[&[2, 1, 0]], 32.0);
}

#[test]
fn small_cogs_do_not_mesh_along_their_axis() {
    let w = world(&[
        ([0, 0, 0], Kind::Cogwheel, Axis::Y),
        ([0, 1, 0], Kind::Cogwheel, Axis::Y),
        ([0, 2, 0], Kind::CreativeMotor, Axis::Y),
    ]);
    // Along Y they are joined by the shaft through them, not by teeth.
    let s = solve(&w);
    assert_eq!(s.speeds[&[0, 0, 0]], 256.0);
}

#[test]
fn large_to_small_doubles_and_small_to_large_halves() {
    let w = world(&[
        ([0, 0, 0], Kind::WaterWheel, Axis::Y),
        ([0, 1, 0], Kind::LargeCogwheel, Axis::Y),
        ([1, 1, 1], Kind::Cogwheel, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.speeds[&[0, 1, 0]], 8.0);
    assert_eq!(s.speeds[&[1, 1, 1]], -16.0);

    let w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::Y),
        ([0, 1, 0], Kind::Cogwheel, Axis::Y),
        ([1, 1, 1], Kind::LargeCogwheel, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.speeds[&[1, 1, 1]], -16.0);
}

#[test]
fn a_large_cog_does_not_mesh_with_a_small_one_beside_it() {
    let w = world(&[
        ([0, 0, 0], Kind::LargeCogwheel, Axis::Y),
        ([1, 0, 0], Kind::Cogwheel, Axis::Y),
    ]);
    assert_eq!(solve(&w).networks.len(), 2);
}

#[test]
fn too_much_stress_stops_the_network() {
    // 8 SU * 32 RPM = 256 SU; two presses want 2 * 8 * 32 = 512 SU.
    let mut blocks = vec![
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::MechanicalPress, Axis::X),
    ];
    let s = solve(&world(&blocks));
    assert_eq!(s.networks[0].state, State::Running);
    assert_eq!(s.networks[0].stress, 256.0);
    blocks.push(([2, 0, 0], Kind::MechanicalPress, Axis::X));
    let s = solve(&world(&blocks));
    assert_eq!(s.networks[0].state, State::Overstressed);
    assert_eq!(s.networks[0].stress, 512.0);
    assert!(s.speeds.values().all(|v| *v == 0.0));
}

#[test]
fn two_sources_add_their_capacity() {
    let w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
        ([2, 0, 0], Kind::HandCrank, Axis::X),
        ([3, 0, 0], Kind::MechanicalPress, Axis::X),
        ([4, 0, 0], Kind::MechanicalPress, Axis::X),
    ]);
    let s = solve(&w);
    assert_eq!(s.networks[0].capacity, 512.0);
    assert_eq!(s.networks[0].state, State::Running);
}

#[test]
fn a_slower_source_in_the_same_direction_is_overpowered() {
    let w = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::X),
        ([1, 0, 0], Kind::WaterWheel, Axis::X),
        ([2, 0, 0], Kind::MechanicalPress, Axis::X),
    ]);
    let s = solve(&w);
    assert_eq!(s.networks[0].state, State::Running);
    assert_eq!(s.speeds[&[1, 0, 0]], 256.0);
    assert_eq!(s.networks[0].capacity, 16384.0 * 256.0);
}

#[test]
fn sources_pushing_against_each_other_conflict() {
    let mut w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::HandCrank, Axis::X),
    ]);
    w.get_mut(&[1, 0, 0]).unwrap().direction = -1;
    let s = solve(&w);
    assert_eq!(s.networks[0].state, State::Conflict);
    assert_eq!(s.speeds[&[0, 0, 0]], 0.0);
}

#[test]
fn a_ring_of_small_cogs_turns() {
    // Cogs on a grid always close in even rings, so the signs agree all the
    // way round.
    let ring: [Pos; 6] = [
        [0, 0, 0],
        [1, 0, 0],
        [2, 0, 0],
        [2, 0, 1],
        [1, 0, 1],
        [0, 0, 1],
    ];
    let mut blocks: Vec<(Pos, Kind, Axis)> =
        ring.iter().map(|p| (*p, Kind::Cogwheel, Axis::Y)).collect();
    blocks.push(([0, 1, 0], Kind::HandCrank, Axis::Y));
    let s = solve(&world(&blocks));
    assert_eq!(s.networks[0].state, State::Running);
    assert_eq!(s.speeds[&[1, 0, 1]], 32.0);
}

#[test]
fn two_large_cogs_share_a_small_one() {
    // The small cog is diagonal to both large cogs: 32 -> -64 -> 32.
    let w = world(&[
        ([0, 0, 0], Kind::LargeCogwheel, Axis::Y),
        ([1, 0, 1], Kind::Cogwheel, Axis::Y),
        ([2, 0, 0], Kind::LargeCogwheel, Axis::Y),
        ([0, 1, 0], Kind::HandCrank, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.speeds[&[1, 0, 1]], -64.0);
    assert_eq!(s.speeds[&[2, 0, 0]], 32.0);
}

#[test]
fn speeds_do_not_depend_on_insertion_order() {
    let a = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
        ([2, 0, 0], Kind::CreativeMotor, Axis::X),
    ]);
    let s1 = solve(&a);
    let mut b = BTreeMap::new();
    for (p, n) in a.iter().rev() {
        b.insert(*p, *n);
    }
    let s2 = solve(&b);
    assert_eq!(s1.speeds, s2.speeds);
}

#[test]
fn an_inactive_source_is_idle() {
    let mut w = world(&[
        ([0, 0, 0], Kind::WaterWheel, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
    ]);
    w.get_mut(&[0, 0, 0]).unwrap().active = false;
    let s = solve(&w);
    assert_eq!(s.networks[0].state, State::Idle);
    assert_eq!(s.speeds[&[1, 0, 0]], 0.0);
}

#[test]
fn bevel_gears_have_reciprocal_signs_for_every_pair_of_axes() {
    for a in [Axis::X, Axis::Y, Axis::Z] {
        for b in [Axis::X, Axis::Y, Axis::Z] {
            if a == b {
                continue;
            }
            for sa in [-1, 1] {
                for sb in [-1, 1] {
                    let mut d = [0; 3];
                    d[a.index()] = sa;
                    d[b.index()] = sb;
                    let na = Node::new(Kind::LargeCogwheel, a);
                    let nb = Node::new(Kind::LargeCogwheel, b);
                    let m = modifier(&na, &nb, d).unwrap();
                    assert_eq!(m, if sa == sb { 1.0 } else { -1.0 });
                    assert_eq!(m * modifier(&nb, &na, d.map(|n| -n)).unwrap(), 1.0);
                }
            }
        }
    }
}
#[test]
fn disabled_clutch_splits_and_reengaging_merges() {
    let mut w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Clutch, Axis::X),
        ([2, 0, 0], Kind::Shaft, Axis::X),
    ]);
    assert_eq!(solve(&w).speeds[&[2, 0, 0]], 32.0);
    w.get_mut(&[1, 0, 0]).unwrap().active = false;
    assert_eq!(solve(&w).speeds[&[2, 0, 0]], 0.0);
    assert_eq!(solve(&w).networks.len(), 3);
    w.get_mut(&[1, 0, 0]).unwrap().active = true;
    assert_eq!(solve(&w).speeds[&[2, 0, 0]], 32.0);
}
#[test]
fn gearshift_reverses_only_the_output_half() {
    let mut w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Gearshift, Axis::X),
        ([2, 0, 0], Kind::Shaft, Axis::X),
    ]);
    w.get_mut(&[1, 0, 0]).unwrap().direction = -1;
    let s = solve(&w);
    assert_eq!(s.speeds[&[0, 0, 0]], 32.0);
    assert_eq!(s.speeds[&[2, 0, 0]], -32.0);
}
#[test]
fn missing_shaft_port_prevents_a_connection() {
    let mut w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
    ]);
    w.get_mut(&[0, 0, 0]).unwrap().ports = 1; // -X only
    assert_eq!(solve(&w).networks.len(), 2);
}
#[test]
fn source_priority_accounts_for_all_gear_ratios() {
    let mut w = world(&[
        ([0, 0, 0], Kind::WaterWheel, Axis::Y),
        ([0, 1, 0], Kind::LargeCogwheel, Axis::Y),
        ([1, 1, 1], Kind::Cogwheel, Axis::Y),
        ([1, 2, 1], Kind::LargeCogwheel, Axis::Y),
        ([2, 2, 2], Kind::Cogwheel, Axis::Y),
        ([2, 3, 2], Kind::LargeCogwheel, Axis::Y),
        ([3, 3, 3], Kind::Cogwheel, Axis::Y),
        ([3, 4, 3], Kind::HandCrank, Axis::Y),
    ]);
    w.get_mut(&[3, 4, 3]).unwrap().direction = -1;
    let s = solve(&w);
    assert_eq!(s.networks.len(), 1);
    assert_eq!(s.networks[0].state, State::Running);
    assert_eq!(s.speeds[&[3, 4, 3]], -64.0);
    assert_eq!(s.networks[0].capacity, 256.0);
}
#[test]
fn speed_up_beyond_the_cap_stops_every_member() {
    let w = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::Y),
        ([0, 1, 0], Kind::LargeCogwheel, Axis::Y),
        ([1, 1, 1], Kind::Cogwheel, Axis::Y),
    ]);
    let s = solve(&w);
    assert_eq!(s.networks[0].state, State::Conflict);
    assert!(s.speeds.values().all(|n| *n == 0.0));
}
#[test]
fn removing_a_bridge_and_a_source_recomputes_components() {
    let mut w = world(&[
        ([0, 0, 0], Kind::HandCrank, Axis::X),
        ([1, 0, 0], Kind::Shaft, Axis::X),
        ([2, 0, 0], Kind::Millstone, Axis::X),
    ]);
    assert_eq!(solve(&w).networks.len(), 1);
    w.remove(&[1, 0, 0]);
    assert_eq!(solve(&w).networks.len(), 2);
    assert_eq!(solve(&w).speeds[&[2, 0, 0]], 0.0);
    w.remove(&[0, 0, 0]);
    assert_eq!(solve(&w).networks[0].state, State::Idle);
}
#[test]
fn extreme_coordinates_do_not_wrap_or_panic() {
    let w = world(&[
        ([i32::MAX, 0, 0], Kind::CreativeMotor, Axis::X),
        ([i32::MIN, 0, 0], Kind::Shaft, Axis::X),
    ]);
    assert_eq!(solve(&w).networks.len(), 2);
    assert_eq!(
        modifier(
            &Node::new(Kind::Shaft, Axis::X),
            &Node::new(Kind::Shaft, Axis::X),
            [i32::MIN, 0, 0]
        ),
        None
    );
}
#[test]
fn malformed_states_are_rejected_instead_of_silently_becoming_y() {
    use crate::state::{Registry, parse_state};
    let r = Registry::new();
    for s in [
        "create:shaft[axis=q]",
        "create:shaft[axis=x",
        "create:shaft[axis=x,axis=y]",
        "create:shaft[=x]",
        "create:clutch[powered=maybe]",
    ] {
        assert!(parse_state(&r, s).is_none(), "{s}");
    }
}

#[test]
fn a_long_reduction_train_does_not_lose_its_driver_to_float_overflow() {
    let mut w = BTreeMap::new();
    for i in 0..180 {
        w.insert([i, i, i], Node::new(Kind::LargeCogwheel, Axis::Y));
        w.insert([i + 1, i, i + 1], Node::new(Kind::Cogwheel, Axis::Y));
    }
    w.insert([180, 180, 180], Node::new(Kind::LargeCogwheel, Axis::Y));
    w.insert([180, 181, 180], Node::new(Kind::HandCrank, Axis::Y));
    let s = solve(&w);
    assert_eq!(s.networks.len(), 1);
    assert_eq!(s.networks[0].state, State::Running);
    assert_eq!(s.speeds[&[180, 181, 180]], 32.0);
}

#[test]
fn gearbox_routes_four_ports_in_plane() {
    let nodes = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::X),
        ([1, 0, 0], Kind::Gearbox, Axis::Y),
        ([1, 0, 1], Kind::Shaft, Axis::Z),
        ([1, 0, -1], Kind::Shaft, Axis::Z),
        ([1, 1, 0], Kind::Shaft, Axis::Y),
    ]);
    let s = solve(&nodes);
    assert_eq!(s.speeds[&[1, 0, 1]], -256.);
    assert_eq!(s.speeds[&[1, 0, -1]], 256.);
    assert_eq!(s.speeds[&[1, 1, 0]], 0.);
}
#[test]
fn parallel_chain_drives_relay_rotation() {
    let nodes = world(&[
        ([0, 0, 0], Kind::CreativeMotor, Axis::X),
        ([1, 0, 0], Kind::EncasedChainDrive, Axis::X),
        ([1, 0, 1], Kind::EncasedChainDrive, Axis::X),
        ([2, 0, 1], Kind::Shaft, Axis::X),
    ]);
    assert_eq!(solve(&nodes).speeds[&[2, 0, 1]], 256.);
}

#[test]
fn configured_source_speed_clamps_and_nonfinite_stops() {
    let mut motor = Node::new(Kind::CreativeMotor, Axis::X);
    motor.configured_rpm = Some(32.0);
    let p = [0, 0, 0];
    let mut nodes = BTreeMap::from([(p, motor)]);
    assert_eq!(solve(&nodes).speeds[&p], 32.0);
    nodes.get_mut(&p).unwrap().configured_rpm = Some(999.0);
    assert_eq!(solve(&nodes).speeds[&p], 256.0);
    nodes.get_mut(&p).unwrap().configured_rpm = Some(f32::NAN);
    assert_eq!(solve(&nodes).speeds[&p], 0.0);
}

}

}
#[allow(dead_code)]
pub mod recipes {
// A recipe laboratory, intentionally separate from player inventories.
// Recipes and RNG are not claimed to be byte-for-byte Java parity. Items
// here have IDs and counts; NBT/components and world automation need host APIs.
use std::collections::BTreeMap;

pub mod catalog {
// Generated from Create 6.0.10 MIT recipe data. Do not edit by hand.
use super::{Heat, Ingredient, Product, Recipe};
pub fn recipes() -> &'static [Recipe] {
    &[
        Recipe {
            id: "create:compacting/andesite_from_flint",
            station: "compacting",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:gravel",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:lava",
                    count: 100,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:andesite",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:compacting/chocolate",
            station: "compacting",
            ingredients: &[Ingredient {
                matcher: "create:chocolate",
                count: 250,
                fluid: true,
                consume: true,
            }],
            products: &[Product {
                id: "create:bar_of_chocolate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:compacting/diorite_from_flint",
            station: "compacting",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:calcite",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:lava",
                    count: 100,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:diorite",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:compacting/granite_from_flint",
            station: "compacting",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:red_sand",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:lava",
                    count: 100,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:granite",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:compacting/honey",
            station: "compacting",
            ingredients: &[Ingredient {
                matcher: "#c:honey",
                count: 1000,
                fluid: true,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:honey_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:compacting/ice",
            station: "compacting",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:snow_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:ice",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/amethyst_block",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:amethyst_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:amethyst_shard",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:amethyst_shard",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/amethyst_cluster",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:amethyst_cluster",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:amethyst_shard",
                    count: 7,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:amethyst_shard",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/asurine",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:asurine",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 0.3,
                },
                Product {
                    id: "create:zinc_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.3,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/asurine_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/asurine",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 0.3,
                },
                Product {
                    id: "create:zinc_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.3,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/blaze_rod",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:blaze_rod",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:blaze_powder",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:blaze_powder",
                    count: 3,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/coal_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:coal_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:coal",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:coal",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/copper_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:copper_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_copper",
                    count: 5,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_copper",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/crimsite",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:crimsite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 0.4,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.4,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/crimsite_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/crimsite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 0.4,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.4,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_coal_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_coal_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:coal",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:coal",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 300,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_copper_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_copper_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_copper",
                    count: 7,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_copper",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_diamond_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_diamond_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:diamond",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:diamond",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 450,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_emerald_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_emerald_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:emerald",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:emerald",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 450,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_gold_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_gold_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_gold",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 2,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_iron_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_iron_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_iron",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_lapis_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_lapis_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:lapis_lazuli",
                    count: 12,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lapis_lazuli",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_redstone_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate_redstone_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:redstone",
                    count: 7,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:redstone",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/deepslate_zinc_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:deepslate_zinc_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobbled_deepslate",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/diamond_horse_armor",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:diamond_horse_armor",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:diamond",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:diamond",
                    count: 3,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:string",
                    count: 2,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/diamond_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:diamond_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:diamond",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:diamond",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/diorite",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:diorite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:quartz",
                count: 1,
                fluid: false,
                chance: 0.25,
            }],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/diorite_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/diorite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:quartz",
                count: 1,
                fluid: false,
                chance: 0.25,
            }],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/emerald_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:emerald_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:emerald",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:emerald",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/gilded_blackstone",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:gilded_blackstone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:gold_nugget",
                    count: 18,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:blackstone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/glowstone",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:glowstone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:glowstone_dust",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:glowstone_dust",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/gold_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:gold_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 2,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/golden_horse_armor",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:golden_horse_armor",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:gold_ingot",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:gold_ingot",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:string",
                    count: 2,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 8,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/gravel",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:gravel",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:sand",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:clay_ball",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/iron_horse_armor",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:iron_horse_armor",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:iron_ingot",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:leather",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:iron_ingot",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:string",
                    count: 2,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 4,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/iron_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:iron_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/lapis_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:lapis_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:lapis_lazuli",
                    count: 10,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lapis_lazuli",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/leather_horse_armor",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:leather_horse_armor",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/nether_gold_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:nether_gold_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:gold_nugget",
                    count: 18,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:netherrack",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/nether_quartz_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:nether_quartz_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:quartz",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:quartz",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:netherrack",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/nether_wart_block",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:nether_wart_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:nether_wart",
                count: 1,
                fluid: false,
                chance: 0.25,
            }],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/netherrack",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:netherrack",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:cinder_flour",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:cinder_flour",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/obsidian",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:obsidian",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:powdered_obsidian",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:obsidian",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 500,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/ochrum",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:ochrum",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 0.2,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.2,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/ochrum_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/ochrum",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 0.2,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.2,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/prismarine_crystals",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:prismarine_crystals",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:quartz",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:quartz",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:glowstone_dust",
                    count: 2,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/raw_copper",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#c:raw_materials/copper",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_copper",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/raw_gold",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#c:raw_materials/gold",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_gold",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 2,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/raw_iron",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#c:raw_materials/iron",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_iron",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/raw_zinc",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#c:raw_materials/zinc",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/raw_zinc_block",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#c:storage_blocks/raw_zinc",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 9,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 9,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 400,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/redstone_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:redstone_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:redstone",
                    count: 6,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:redstone",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/tuff",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "minecraft:tuff",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "create:copper_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "create:zinc_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/tuff_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/tuff",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "create:copper_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "create:zinc_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 350,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/veridium",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:veridium",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_copper",
                    count: 1,
                    fluid: false,
                    chance: 0.8,
                },
                Product {
                    id: "create:copper_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.8,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/veridium_recycling",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#create:stone_types/veridium",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_copper",
                    count: 1,
                    fluid: false,
                    chance: 0.8,
                },
                Product {
                    id: "create:copper_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.8,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/wool",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "#minecraft:wool",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:string",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:string",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:crushing/zinc_ore",
            station: "crushing",
            ingredients: &[Ingredient {
                matcher: "create:zinc_ore",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:crushed_raw_zinc",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "create:experience_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
                Product {
                    id: "minecraft:cobblestone",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:cutting/andesite_alloy",
            station: "cutting",
            ingredients: &[Ingredient {
                matcher: "create:andesite_alloy",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:shaft",
                count: 6,
                fluid: false,
                chance: 1.0,
            }],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:cutting/bamboo_planks",
            station: "cutting",
            ingredients: &[Ingredient {
                matcher: "minecraft:bamboo_planks",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:bamboo_mosaic",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 20,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/chiseled_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/chiseled_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cogwheel",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:shaft",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:planks",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:cogwheel",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_block_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_block_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_copper_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_bulb_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_bulb_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_door_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_door_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_grate_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_grate_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingle_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingle_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingle_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingle_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_shingles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tile_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tile_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tile_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tile_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tiles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_tiles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_trapdoor_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/copper_trapdoor_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/cut_copper_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_chiseled_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_chiseled_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_bulb_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_bulb_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_door_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_door_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_grate_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_grate_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingle_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingle_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingle_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingle_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_shingles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tile_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tile_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tile_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tile_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tiles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_tiles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_exposed_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:exposed_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_trapdoor_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_copper_trapdoor_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/exposed_cut_copper_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_exposed_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:exposed_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/large_cogwheel",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:cogwheel",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:planks",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:large_cogwheel",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_chiseled_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_bulb_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_door_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_grate_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_shingle_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_shingle_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_shingles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_tile_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_tile_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_tiles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_oxidized_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:oxidized_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_copper_trapdoor_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_cut_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_cut_copper_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/oxidized_cut_copper_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_oxidized_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:oxidized_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_chiseled_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_block_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:copper_block",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_copper_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_bulb_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_door_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_grate_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_shingle_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_shingle_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_shingles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_tile_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_tile_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_tiles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_copper_trapdoor_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_cut_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_cut_copper_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_cut_copper_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_chiseled_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_bulb_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_door_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_grate_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_shingle_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_shingle_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_shingles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_tile_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_tile_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_tiles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:exposed_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_exposed_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_copper_trapdoor_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_cut_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_cut_copper_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_exposed_cut_copper_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:exposed_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_exposed_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_chiseled_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_bulb_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_door_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_grate_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_shingle_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_shingle_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_shingles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_tile_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_tile_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_tiles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_oxidized_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_copper_trapdoor_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_cut_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_cut_copper_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_oxidized_cut_copper_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_oxidized_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_chiseled_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_bulb_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_door_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_grate_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_shingle_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_shingle_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_shingles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_tile_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_tile_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_tiles_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:weathered_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:waxed_weathered_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_copper_trapdoor_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_cut_copper_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_cut_copper_slab_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/waxed_weathered_cut_copper_stairs_from_adding_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:weathered_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:honeycomb_block",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:waxed_weathered_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_chiseled_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_chiseled_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_chiseled_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_chiseled_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_bulb_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_bulb_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_copper_bulb",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_bulb",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_door_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_door_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_copper_door",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_door",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_grate_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_grate_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_copper_grate",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_grate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingle_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingle_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_shingle_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingle_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingle_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingle_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_shingle_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingle_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_shingles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_shingles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_shingles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tile_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tile_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_tile_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tile_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tile_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tile_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_tile_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tile_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tiles_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:oxidized_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_tiles_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "create:waxed_weathered_copper_tiles",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "create:weathered_copper_tiles",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_trapdoor_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_copper_trapdoor_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_copper_trapdoor",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_copper_trapdoor",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_cut_copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_slab_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_slab_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_cut_copper_slab",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper_slab",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_stairs_from_deoxidising",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:oxidized_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:deploying/weathered_cut_copper_stairs_from_removing_wax",
            station: "deploying",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:waxed_weathered_cut_copper_stairs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#minecraft:axes",
                    count: 1,
                    fluid: false,
                    consume: false,
                },
            ],
            products: &[Product {
                id: "minecraft:weathered_cut_copper_stairs",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/builders_tea",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "create:builders_tea",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:glass_bottle",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:tea",
                    count: 250,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/honey_bottle",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "minecraft:honey_bottle",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:glass_bottle",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:honey",
                    count: 250,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/blaze_cake",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "create:blaze_cake_base",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:lava",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:blaze_cake",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/builders_tea",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:glass_bottle",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:tea",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:builders_tea",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/chocolate_glazed_berries",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:sweet_berries",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:chocolate",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:chocolate_glazed_berries",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/grass_block",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:dirt",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:water",
                    count: 500,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:grass_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/honey_bottle",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:glass_bottle",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:honey",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:honey_bottle",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/honeyed_apple",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:apple",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:honey",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:honeyed_apple",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/blackstone",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "#c:cobblestones",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:blackstone",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/crimson_fungus",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:red_mushroom",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:crimson_fungus",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/glow_berries",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:sweet_berries",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:glow_berries",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/glow_ink_sac",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:ink_sac",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:glow_ink_sac",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/haunted_bell",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "create:peculiar_bell",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:haunted_bell",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_chiseled_stone_bricks",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:chiseled_stone_bricks",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_chiseled_stone_bricks",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_cracked_stone_bricks",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:cracked_stone_bricks",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_cracked_stone_bricks",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_deepslate",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:deepslate",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_deepslate",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_mossy_stone_bricks",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:mossy_stone_bricks",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_mossy_stone_bricks",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_stone",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:stone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_stone",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/infested_stone_bricks",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:stone_bricks",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:infested_stone_bricks",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/nether_brick",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:brick",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:nether_brick",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/poisonous_potato",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:potato",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:poisonous_potato",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/soul_campfire",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:campfire",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:soul_campfire",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/soul_lantern",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:lantern",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:soul_lantern",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/soul_torch",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:torch",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:soul_torch",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:haunting/warped_fungus",
            station: "haunting",
            ingredients: &[Ingredient {
                matcher: "minecraft:brown_mushroom",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:warped_fungus",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/andesite_casing_from_log",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_logs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:andesite_alloy",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:andesite_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/andesite_casing_from_wood",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_woods",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:andesite_alloy",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:andesite_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/brass_casing_from_log",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_logs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:ingots/brass",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:brass_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/brass_casing_from_wood",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_woods",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:ingots/brass",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:brass_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/copper_casing_from_log",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_logs",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:ingots/copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:copper_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/copper_casing_from_wood",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "#c:stripped_woods",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:ingots/copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:copper_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:item_application/railway_casing",
            station: "item_application",
            ingredients: &[
                Ingredient {
                    matcher: "create:brass_casing",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:plates/obsidian",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:railway_casing",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/allium",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:allium",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:magenta_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:purple_dye",
                    count: 2,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:pink_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/andesite",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:andesite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:cobblestone",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/azure_bluet",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:azure_bluet",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:light_gray_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:white_dye",
                    count: 2,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/beetroot",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:beetroot",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:red_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:beetroot_seeds",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 70,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/blue_orchid",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:blue_orchid",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:light_blue_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:light_gray_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/bone",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:bone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bone_meal",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:white_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:bone_meal",
                    count: 3,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/bone_meal",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:bone_meal",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:white_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:light_gray_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 70,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/cactus",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:cactus",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:green_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/calcite",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:calcite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:bone_meal",
                count: 1,
                fluid: false,
                chance: 0.75,
            }],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/charcoal",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:charcoal",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:black_dye",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:gray_dye",
                    count: 2,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/clay",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:clay",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:clay_ball",
                count: 4,
                fluid: false,
                chance: 1.0,
            }],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/coal",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:coal",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:black_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:gray_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/cobblestone",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:cobblestone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:gravel",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/cocoa_beans",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:cocoa_beans",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:brown_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:brown_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 70,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/cornflower",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:cornflower",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:blue_dye",
                count: 2,
                fluid: false,
                chance: 1.0,
            }],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/dandelion",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:dandelion",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:yellow_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:yellow_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/dripstone_block",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:dripstone_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:clay_ball",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/fern",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:fern",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:wheat_seeds",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/granite",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:granite",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:red_sand",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/gravel",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:gravel",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:flint",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 250,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/ink_sac",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:ink_sac",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:black_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:gray_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/lapis_lazuli",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:lapis_lazuli",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:blue_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:blue_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/large_fern",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:large_fern",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:green_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
                Product {
                    id: "minecraft:wheat_seeds",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/lilac",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:lilac",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:magenta_dye",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:magenta_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:purple_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/lily_of_the_valley",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:lily_of_the_valley",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:white_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
                Product {
                    id: "minecraft:white_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/orange_tulip",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:orange_tulip",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:orange_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/oxeye_daisy",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:oxeye_daisy",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:light_gray_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:white_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.2,
                },
                Product {
                    id: "minecraft:yellow_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/peony",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:peony",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:pink_dye",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:magenta_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:pink_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/pink_petals",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:pink_petals",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:pink_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/pink_tulip",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:pink_tulip",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:pink_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/pitcher_plant",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:pitcher_plant",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:cyan_dye",
                    count: 4,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:purple_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/poppy",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:poppy",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:red_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/red_tulip",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:red_tulip",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:red_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/rose_bush",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:rose_bush",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:red_dye",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 2,
                    fluid: false,
                    chance: 0.05,
                },
                Product {
                    id: "minecraft:red_dye",
                    count: 2,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/saddle",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:saddle",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:leather",
                    count: 2,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/sandstone",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:sandstone",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:sand",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/sea_pickle",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:sea_pickle",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:lime_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/short_grass",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:short_grass",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:wheat_seeds",
                count: 1,
                fluid: false,
                chance: 0.25,
            }],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/sugar_cane",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:sugar_cane",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:sugar",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:sugar",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/sunflower",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:sunflower",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:yellow_dye",
                    count: 3,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:orange_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:yellow_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/tall_grass",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:tall_grass",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:wheat_seeds",
                count: 1,
                fluid: false,
                chance: 0.5,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/terracotta",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:terracotta",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:red_sand",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 200,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/torchflower",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:torchflower",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:orange_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:green_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/wheat",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:wheat",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:wheat_flour",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:wheat_flour",
                    count: 2,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:wheat_seeds",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 150,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/white_tulip",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:white_tulip",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:white_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lime_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/wither_rose",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "minecraft:wither_rose",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:black_dye",
                    count: 2,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:black_dye",
                    count: 1,
                    fluid: false,
                    chance: 0.1,
                },
            ],
            work: 50,
            heat: Heat::None,
        },
        Recipe {
            id: "create:milling/wool",
            station: "milling",
            ingredients: &[Ingredient {
                matcher: "#minecraft:wool",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:string",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:mixing/andesite_alloy",
            station: "mixing",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:andesite",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:nuggets/iron",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:andesite_alloy",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:mixing/andesite_alloy_from_zinc",
            station: "mixing",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:andesite",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:nuggets/zinc",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:andesite_alloy",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:mixing/brass_ingot",
            station: "mixing",
            ingredients: &[
                Ingredient {
                    matcher: "#c:ingots/copper",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#c:ingots/zinc",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:brass_ingot",
                count: 2,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::Heated,
        },
        Recipe {
            id: "create:mixing/cardboard_pulp",
            station: "mixing",
            ingredients: &[
                Ingredient {
                    matcher: "#create:pulpifiable",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#create:pulpifiable",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#create:pulpifiable",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "#create:pulpifiable",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:water",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:pulp",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:mixing/chocolate_melting",
            station: "mixing",
            ingredients: &[Ingredient {
                matcher: "create:bar_of_chocolate",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:chocolate",
                count: 250,
                fluid: true,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::Heated,
        },
        Recipe {
            id: "create:mixing/dough_by_mixing",
            station: "mixing",
            ingredients: &[
                Ingredient {
                    matcher: "#c:flours/wheat",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:water",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:dough",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:mixing/honey",
            station: "mixing",
            ingredients: &[Ingredient {
                matcher: "minecraft:honey_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:honey",
                count: 1000,
                fluid: true,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::Heated,
        },
        Recipe {
            id: "create:mixing/lava_from_cobble",
            station: "mixing",
            ingredients: &[Ingredient {
                matcher: "#c:cobblestones",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:lava",
                count: 50,
                fluid: true,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::Superheated,
        },
        Recipe {
            id: "create:pressing/brass_ingot",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "#c:ingots/brass",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:brass_sheet",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/cardboard",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "create:pulp",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:cardboard",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/copper_ingot",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "#c:ingots/copper",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:copper_sheet",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/gold_ingot",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "#c:ingots/gold",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:golden_sheet",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/iron_ingot",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "#c:ingots/iron",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:iron_sheet",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/path_from_grass",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "minecraft:grass_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:dirt_path",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:pressing/sugar_cane",
            station: "pressing",
            ingredients: &[Ingredient {
                matcher: "minecraft:sugar_cane",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:paper",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:sandpaper_polishing/rose_quartz",
            station: "sandpaper_polishing",
            ingredients: &[Ingredient {
                matcher: "create:rose_quartz",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:polished_rose_quartz",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/crushed_raw_copper",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:crushed_raw_copper",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:copper_nugget",
                    count: 9,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:clay_ball",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/crushed_raw_gold",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:crushed_raw_gold",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:gold_nugget",
                    count: 9,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:quartz",
                    count: 1,
                    fluid: false,
                    chance: 0.5,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/crushed_raw_iron",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:crushed_raw_iron",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:iron_nugget",
                    count: 9,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:redstone",
                    count: 1,
                    fluid: false,
                    chance: 0.75,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/crushed_raw_zinc",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:crushed_raw_zinc",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "create:zinc_nugget",
                    count: 9,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:gunpowder",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/gravel",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:gravel",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:flint",
                    count: 1,
                    fluid: false,
                    chance: 0.25,
                },
                Product {
                    id: "minecraft:iron_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.125,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/ice",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:ice",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:packed_ice",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/industrial_iron_block",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:industrial_iron_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:weathered_iron_block",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/industrial_iron_window",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:industrial_iron_window",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:weathered_iron_window",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/industrial_iron_window_pane",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "create:industrial_iron_window_pane",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:weathered_iron_window_pane",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/magma_block",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:magma_block",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:obsidian",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/red_sand",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:red_sand",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:gold_nugget",
                    count: 3,
                    fluid: false,
                    chance: 0.125,
                },
                Product {
                    id: "minecraft:dead_bush",
                    count: 1,
                    fluid: false,
                    chance: 0.05,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/sand",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:sand",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:clay_ball",
                count: 1,
                fluid: false,
                chance: 0.25,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/soul_sand",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "minecraft:soul_sand",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:quartz",
                    count: 4,
                    fluid: false,
                    chance: 0.125,
                },
                Product {
                    id: "minecraft:gold_nugget",
                    count: 1,
                    fluid: false,
                    chance: 0.02,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/stained_glass_pane",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "#c:glass_panes",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:glass_pane",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/wheat_flour",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "#c:flours/wheat",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "create:dough",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:splashing/wool",
            station: "splashing",
            ingredients: &[Ingredient {
                matcher: "#minecraft:wool",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[Product {
                id: "minecraft:white_wool",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
    ]
}
pub fn tags() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        (
            "c:buckets",
            &["create:chocolate_bucket", "create:honey_bucket"],
        ),
        ("c:buckets/chocolate", &["create:chocolate_bucket"]),
        ("c:buckets/honey", &["create:honey_bucket"]),
        (
            "c:chocolate",
            &["create:chocolate", "create:flowing_chocolate"],
        ),
        (
            "c:cobblestones",
            &["minecraft:cobbled_deepslate", "minecraft:cobblestone"],
        ),
        ("c:drinks", &["create:builders_tea"]),
        ("c:drinks/tea", &["create:builders_tea"]),
        ("c:dusts/obsidian", &["create:powdered_obsidian"]),
        (
            "c:enchantables",
            &[
                "create:copper_backtank",
                "create:netherite_backtank",
                "create:potato_cannon",
            ],
        ),
        ("c:flours", &["create:wheat_flour"]),
        ("c:flours/wheat", &["create:wheat_flour"]),
        (
            "c:foods",
            &[
                "create:bar_of_chocolate",
                "create:builders_tea",
                "create:chocolate_glazed_berries",
                "create:honeyed_apple",
                "create:sweet_roll",
            ],
        ),
        ("c:foods/berry", &["create:chocolate_glazed_berries"]),
        ("c:foods/chocolate", &["create:bar_of_chocolate"]),
        ("c:foods/dough", &["create:dough"]),
        ("c:foods/dough/wheat", &["create:dough"]),
        ("c:foods/fruit", &["create:honeyed_apple"]),
        (
            "c:glass_blocks/colorless",
            &[
                "create:framed_glass",
                "create:horizontal_framed_glass",
                "create:tiled_glass",
                "create:vertical_framed_glass",
            ],
        ),
        (
            "c:glass_panes",
            &[
                "create:acacia_window_pane",
                "create:bamboo_window_pane",
                "create:birch_window_pane",
                "create:cherry_window_pane",
                "create:crimson_window_pane",
                "create:dark_oak_window_pane",
                "create:framed_glass_pane",
                "create:horizontal_framed_glass_pane",
                "create:industrial_iron_window_pane",
                "create:jungle_window_pane",
                "create:mangrove_window_pane",
                "create:oak_window_pane",
                "create:ornate_iron_window_pane",
                "create:spruce_window_pane",
                "create:tiled_glass_pane",
                "create:vertical_framed_glass_pane",
                "create:warped_window_pane",
                "create:weathered_iron_window_pane",
            ],
        ),
        (
            "c:glass_panes/colorless",
            &[
                "create:framed_glass_pane",
                "create:horizontal_framed_glass_pane",
                "create:tiled_glass_pane",
                "create:vertical_framed_glass_pane",
            ],
        ),
        ("c:honey", &["create:flowing_honey", "create:honey"]),
        (
            "c:ingots",
            &[
                "create:andesite_alloy",
                "create:brass_ingot",
                "create:zinc_ingot",
            ],
        ),
        ("c:ingots/brass", &["create:brass_ingot"]),
        ("c:ingots/copper", &["minecraft:copper_ingot"]),
        ("c:ingots/gold", &["minecraft:gold_ingot"]),
        ("c:ingots/iron", &["minecraft:iron_ingot"]),
        ("c:ingots/zinc", &["create:zinc_ingot"]),
        ("c:lava", &["minecraft:lava"]),
        (
            "c:nuggets",
            &[
                "create:brass_nugget",
                "create:copper_nugget",
                "create:experience_nugget",
                "create:zinc_nugget",
            ],
        ),
        ("c:nuggets/brass", &["create:brass_nugget"]),
        ("c:nuggets/copper", &["create:copper_nugget"]),
        ("c:nuggets/gold", &["minecraft:gold_nugget"]),
        ("c:nuggets/iron", &["minecraft:iron_nugget"]),
        ("c:nuggets/zinc", &["create:zinc_nugget"]),
        ("c:ores", &["create:deepslate_zinc_ore", "create:zinc_ore"]),
        (
            "c:ores/zinc",
            &["create:deepslate_zinc_ore", "create:zinc_ore"],
        ),
        ("c:ores_in_ground/deepslate", &["create:deepslate_zinc_ore"]),
        ("c:ores_in_ground/stone", &["create:zinc_ore"]),
        (
            "c:plates",
            &[
                "create:brass_sheet",
                "create:cardboard",
                "create:copper_sheet",
                "create:golden_sheet",
                "create:iron_sheet",
                "create:sturdy_sheet",
            ],
        ),
        ("c:plates/brass", &["create:brass_sheet"]),
        ("c:plates/cardboard", &["create:cardboard"]),
        ("c:plates/copper", &["create:copper_sheet"]),
        ("c:plates/gold", &["create:golden_sheet"]),
        ("c:plates/iron", &["create:iron_sheet"]),
        ("c:plates/obsidian", &["create:sturdy_sheet"]),
        ("c:raw_materials", &["create:raw_zinc"]),
        ("c:raw_materials/copper", &["minecraft:raw_copper"]),
        ("c:raw_materials/gold", &["minecraft:raw_gold"]),
        ("c:raw_materials/iron", &["minecraft:raw_iron"]),
        ("c:raw_materials/zinc", &["create:raw_zinc"]),
        (
            "c:stones",
            &[
                "minecraft:andesite",
                "minecraft:diorite",
                "minecraft:granite",
                "minecraft:stone",
            ],
        ),
        (
            "c:storage_blocks",
            &[
                "create:andesite_alloy_block",
                "create:brass_block",
                "create:cardboard_block",
                "create:experience_block",
                "create:raw_zinc_block",
                "create:zinc_block",
            ],
        ),
        (
            "c:storage_blocks/andesite_alloy",
            &["create:andesite_alloy_block"],
        ),
        ("c:storage_blocks/brass", &["create:brass_block"]),
        ("c:storage_blocks/cardboard", &["create:cardboard_block"]),
        ("c:storage_blocks/copper", &["minecraft:copper_block"]),
        ("c:storage_blocks/gold", &["minecraft:gold_block"]),
        ("c:storage_blocks/iron", &["minecraft:iron_block"]),
        ("c:storage_blocks/raw_zinc", &["create:raw_zinc_block"]),
        ("c:storage_blocks/zinc", &["create:zinc_block"]),
        (
            "c:stripped_logs",
            &[
                "minecraft:stripped_acacia_log",
                "minecraft:stripped_birch_log",
                "minecraft:stripped_cherry_log",
                "minecraft:stripped_dark_oak_log",
                "minecraft:stripped_jungle_log",
                "minecraft:stripped_mangrove_log",
                "minecraft:stripped_oak_log",
                "minecraft:stripped_pale_oak_log",
                "minecraft:stripped_spruce_log",
            ],
        ),
        (
            "c:stripped_woods",
            &[
                "minecraft:stripped_acacia_wood",
                "minecraft:stripped_birch_wood",
                "minecraft:stripped_cherry_wood",
                "minecraft:stripped_dark_oak_wood",
                "minecraft:stripped_jungle_wood",
                "minecraft:stripped_mangrove_wood",
                "minecraft:stripped_oak_wood",
                "minecraft:stripped_pale_oak_wood",
                "minecraft:stripped_spruce_wood",
            ],
        ),
        ("c:tea", &["create:flowing_tea", "create:tea"]),
        ("c:tools/wrench", &["create:wrench"]),
        ("c:water", &["minecraft:water"]),
        (
            "create:bottomless/allow",
            &["minecraft:lava", "minecraft:water"],
        ),
        (
            "create:casing",
            &[
                "create:andesite_casing",
                "create:brass_casing",
                "create:copper_casing",
                "create:railway_casing",
                "create:refined_radiance_casing",
                "create:shadow_steel_casing",
            ],
        ),
        ("create:chain_rideable", &["create:wrench"]),
        (
            "create:contraption_controlled",
            &[
                "create:andesite_door",
                "create:andesite_funnel",
                "create:brass_door",
                "create:brass_funnel",
                "create:copper_door",
                "create:deployer",
                "create:framed_glass_door",
                "create:haunted_bell",
                "create:mechanical_drill",
                "create:mechanical_harvester",
                "create:mechanical_plough",
                "create:mechanical_roller",
                "create:mechanical_saw",
                "create:peculiar_bell",
                "create:portable_fluid_interface",
                "create:portable_storage_interface",
                "create:redstone_contact",
                "create:train_door",
                "minecraft:bell",
                "minecraft:campfire",
                "minecraft:dispenser",
                "minecraft:dropper",
                "minecraft:soul_campfire",
            ],
        ),
        (
            "create:create_ingots",
            &[
                "create:andesite_alloy",
                "create:brass_ingot",
                "create:zinc_ingot",
            ],
        ),
        (
            "create:crushed_raw_materials",
            &[
                "create:crushed_raw_aluminum",
                "create:crushed_raw_copper",
                "create:crushed_raw_gold",
                "create:crushed_raw_iron",
                "create:crushed_raw_lead",
                "create:crushed_raw_nickel",
                "create:crushed_raw_osmium",
                "create:crushed_raw_platinum",
                "create:crushed_raw_quicksilver",
                "create:crushed_raw_silver",
                "create:crushed_raw_tin",
                "create:crushed_raw_uranium",
                "create:crushed_raw_zinc",
            ],
        ),
        (
            "create:deployable_drink",
            &["minecraft:milk_bucket", "minecraft:potion"],
        ),
        (
            "create:dyed_table_cloths",
            &[
                "create:black_table_cloth",
                "create:blue_table_cloth",
                "create:brown_table_cloth",
                "create:cyan_table_cloth",
                "create:gray_table_cloth",
                "create:green_table_cloth",
                "create:light_blue_table_cloth",
                "create:light_gray_table_cloth",
                "create:lime_table_cloth",
                "create:magenta_table_cloth",
                "create:orange_table_cloth",
                "create:pink_table_cloth",
                "create:purple_table_cloth",
                "create:red_table_cloth",
                "create:white_table_cloth",
                "create:yellow_table_cloth",
            ],
        ),
        (
            "create:fan_processing_catalysts/blasting",
            &["minecraft:flowing_lava", "minecraft:lava"],
        ),
        (
            "create:fan_processing_catalysts/splashing",
            &["minecraft:flowing_water", "minecraft:water"],
        ),
        (
            "create:invalid_for_track_paving",
            &["create:metal_bracket", "create:wooden_bracket"],
        ),
        (
            "create:packages",
            &[
                "create:cardboard_package_10x12",
                "create:cardboard_package_10x8",
                "create:cardboard_package_12x10",
                "create:cardboard_package_12x12",
                "create:rare_creeper_package",
                "create:rare_darcy_package",
                "create:rare_evan_package",
                "create:rare_jinx_package",
                "create:rare_kryppers_package",
                "create:rare_simi_package",
                "create:rare_starlotte_package",
                "create:rare_thunder_package",
                "create:rare_up_package",
                "create:rare_vector_package",
            ],
        ),
        (
            "create:postboxes",
            &[
                "create:black_postbox",
                "create:blue_postbox",
                "create:brown_postbox",
                "create:cyan_postbox",
                "create:gray_postbox",
                "create:green_postbox",
                "create:light_blue_postbox",
                "create:light_gray_postbox",
                "create:lime_postbox",
                "create:magenta_postbox",
                "create:orange_postbox",
                "create:pink_postbox",
                "create:purple_postbox",
                "create:red_postbox",
                "create:white_postbox",
                "create:yellow_postbox",
            ],
        ),
        (
            "create:pressurized_air_sources",
            &["create:copper_backtank", "create:netherite_backtank"],
        ),
        (
            "create:pulpifiable",
            &["minecraft:bamboo", "minecraft:sugar_cane"],
        ),
        (
            "create:sandpaper",
            &["create:red_sand_paper", "create:sand_paper"],
        ),
        (
            "create:seats",
            &[
                "create:black_seat",
                "create:blue_seat",
                "create:brown_seat",
                "create:cyan_seat",
                "create:gray_seat",
                "create:green_seat",
                "create:light_blue_seat",
                "create:light_gray_seat",
                "create:lime_seat",
                "create:magenta_seat",
                "create:orange_seat",
                "create:pink_seat",
                "create:purple_seat",
                "create:red_seat",
                "create:white_seat",
                "create:yellow_seat",
            ],
        ),
        (
            "create:sleepers",
            &[
                "minecraft:andesite_slab",
                "minecraft:smooth_stone_slab",
                "minecraft:stone_slab",
            ],
        ),
        (
            "create:stone_types/andesite",
            &[
                "create:andesite_pillar",
                "create:cut_andesite",
                "create:cut_andesite_brick_stairs",
                "create:cut_andesite_brick_wall",
                "create:cut_andesite_bricks",
                "create:cut_andesite_stairs",
                "create:cut_andesite_wall",
                "create:layered_andesite",
                "create:polished_cut_andesite",
                "create:polished_cut_andesite_stairs",
                "create:polished_cut_andesite_wall",
                "create:small_andesite_brick_stairs",
                "create:small_andesite_brick_wall",
                "create:small_andesite_bricks",
                "minecraft:andesite",
            ],
        ),
        (
            "create:stone_types/asurine",
            &[
                "create:asurine",
                "create:asurine_pillar",
                "create:cut_asurine",
                "create:cut_asurine_brick_stairs",
                "create:cut_asurine_brick_wall",
                "create:cut_asurine_bricks",
                "create:cut_asurine_stairs",
                "create:cut_asurine_wall",
                "create:layered_asurine",
                "create:polished_cut_asurine",
                "create:polished_cut_asurine_stairs",
                "create:polished_cut_asurine_wall",
                "create:small_asurine_brick_stairs",
                "create:small_asurine_brick_wall",
                "create:small_asurine_bricks",
            ],
        ),
        (
            "create:stone_types/calcite",
            &[
                "create:calcite_pillar",
                "create:cut_calcite",
                "create:cut_calcite_brick_stairs",
                "create:cut_calcite_brick_wall",
                "create:cut_calcite_bricks",
                "create:cut_calcite_stairs",
                "create:cut_calcite_wall",
                "create:layered_calcite",
                "create:polished_cut_calcite",
                "create:polished_cut_calcite_stairs",
                "create:polished_cut_calcite_wall",
                "create:small_calcite_brick_stairs",
                "create:small_calcite_brick_wall",
                "create:small_calcite_bricks",
                "minecraft:calcite",
            ],
        ),
        (
            "create:stone_types/crimsite",
            &[
                "create:crimsite",
                "create:crimsite_pillar",
                "create:cut_crimsite",
                "create:cut_crimsite_brick_stairs",
                "create:cut_crimsite_brick_wall",
                "create:cut_crimsite_bricks",
                "create:cut_crimsite_stairs",
                "create:cut_crimsite_wall",
                "create:layered_crimsite",
                "create:polished_cut_crimsite",
                "create:polished_cut_crimsite_stairs",
                "create:polished_cut_crimsite_wall",
                "create:small_crimsite_brick_stairs",
                "create:small_crimsite_brick_wall",
                "create:small_crimsite_bricks",
            ],
        ),
        (
            "create:stone_types/deepslate",
            &[
                "create:cut_deepslate",
                "create:cut_deepslate_brick_stairs",
                "create:cut_deepslate_brick_wall",
                "create:cut_deepslate_bricks",
                "create:cut_deepslate_stairs",
                "create:cut_deepslate_wall",
                "create:deepslate_pillar",
                "create:layered_deepslate",
                "create:polished_cut_deepslate",
                "create:polished_cut_deepslate_stairs",
                "create:polished_cut_deepslate_wall",
                "create:small_deepslate_brick_stairs",
                "create:small_deepslate_brick_wall",
                "create:small_deepslate_bricks",
                "minecraft:deepslate",
            ],
        ),
        (
            "create:stone_types/diorite",
            &[
                "create:cut_diorite",
                "create:cut_diorite_brick_stairs",
                "create:cut_diorite_brick_wall",
                "create:cut_diorite_bricks",
                "create:cut_diorite_stairs",
                "create:cut_diorite_wall",
                "create:diorite_pillar",
                "create:layered_diorite",
                "create:polished_cut_diorite",
                "create:polished_cut_diorite_stairs",
                "create:polished_cut_diorite_wall",
                "create:small_diorite_brick_stairs",
                "create:small_diorite_brick_wall",
                "create:small_diorite_bricks",
                "minecraft:diorite",
            ],
        ),
        (
            "create:stone_types/dripstone",
            &[
                "create:cut_dripstone",
                "create:cut_dripstone_brick_stairs",
                "create:cut_dripstone_brick_wall",
                "create:cut_dripstone_bricks",
                "create:cut_dripstone_stairs",
                "create:cut_dripstone_wall",
                "create:dripstone_pillar",
                "create:layered_dripstone",
                "create:polished_cut_dripstone",
                "create:polished_cut_dripstone_stairs",
                "create:polished_cut_dripstone_wall",
                "create:small_dripstone_brick_stairs",
                "create:small_dripstone_brick_wall",
                "create:small_dripstone_bricks",
                "minecraft:dripstone_block",
            ],
        ),
        (
            "create:stone_types/galosphere/allurite",
            &[
                "galosphere:allurite_block",
                "galosphere:allurite_brick_slab",
                "galosphere:allurite_brick_stairs",
                "galosphere:allurite_bricks",
                "galosphere:allurite_slab",
                "galosphere:allurite_stairs",
                "galosphere:chiseled_allurite",
                "galosphere:smooth_allurite",
                "galosphere:smooth_allurite_slab",
                "galosphere:smooth_allurite_stairs",
            ],
        ),
        (
            "create:stone_types/galosphere/amethyst",
            &[
                "galosphere:amethyst_block",
                "galosphere:amethyst_brick_slab",
                "galosphere:amethyst_brick_stairs",
                "galosphere:amethyst_bricks",
                "galosphere:amethyst_slab",
                "galosphere:amethyst_stairs",
                "galosphere:chiseled_amethyst",
                "galosphere:smooth_amethyst",
                "galosphere:smooth_amethyst_slab",
                "galosphere:smooth_amethyst_stairs",
            ],
        ),
        (
            "create:stone_types/galosphere/lumiere",
            &[
                "galosphere:chiseled_lumiere",
                "galosphere:lumiere_block",
                "galosphere:lumiere_brick_slab",
                "galosphere:lumiere_brick_stairs",
                "galosphere:lumiere_bricks",
                "galosphere:lumiere_slab",
                "galosphere:lumiere_stairs",
                "galosphere:smooth_lumiere",
                "galosphere:smooth_lumiere_slab",
                "galosphere:smooth_lumiere_stairs",
            ],
        ),
        (
            "create:stone_types/granite",
            &[
                "create:cut_granite",
                "create:cut_granite_brick_stairs",
                "create:cut_granite_brick_wall",
                "create:cut_granite_bricks",
                "create:cut_granite_stairs",
                "create:cut_granite_wall",
                "create:granite_pillar",
                "create:layered_granite",
                "create:polished_cut_granite",
                "create:polished_cut_granite_stairs",
                "create:polished_cut_granite_wall",
                "create:small_granite_brick_stairs",
                "create:small_granite_brick_wall",
                "create:small_granite_bricks",
                "minecraft:granite",
            ],
        ),
        (
            "create:stone_types/limestone",
            &[
                "create:cut_limestone",
                "create:cut_limestone_brick_stairs",
                "create:cut_limestone_brick_wall",
                "create:cut_limestone_bricks",
                "create:cut_limestone_stairs",
                "create:cut_limestone_wall",
                "create:layered_limestone",
                "create:limestone",
                "create:limestone_pillar",
                "create:polished_cut_limestone",
                "create:polished_cut_limestone_stairs",
                "create:polished_cut_limestone_wall",
                "create:small_limestone_brick_stairs",
                "create:small_limestone_brick_wall",
                "create:small_limestone_bricks",
            ],
        ),
        (
            "create:stone_types/ochrum",
            &[
                "create:cut_ochrum",
                "create:cut_ochrum_brick_stairs",
                "create:cut_ochrum_brick_wall",
                "create:cut_ochrum_bricks",
                "create:cut_ochrum_stairs",
                "create:cut_ochrum_wall",
                "create:layered_ochrum",
                "create:ochrum",
                "create:ochrum_pillar",
                "create:polished_cut_ochrum",
                "create:polished_cut_ochrum_stairs",
                "create:polished_cut_ochrum_wall",
                "create:small_ochrum_brick_stairs",
                "create:small_ochrum_brick_wall",
                "create:small_ochrum_bricks",
            ],
        ),
        (
            "create:stone_types/scorchia",
            &[
                "create:cut_scorchia",
                "create:cut_scorchia_brick_stairs",
                "create:cut_scorchia_brick_wall",
                "create:cut_scorchia_bricks",
                "create:cut_scorchia_stairs",
                "create:cut_scorchia_wall",
                "create:layered_scorchia",
                "create:polished_cut_scorchia",
                "create:polished_cut_scorchia_stairs",
                "create:polished_cut_scorchia_wall",
                "create:scorchia",
                "create:scorchia_pillar",
                "create:small_scorchia_brick_stairs",
                "create:small_scorchia_brick_wall",
                "create:small_scorchia_bricks",
            ],
        ),
        (
            "create:stone_types/scoria",
            &[
                "create:cut_scoria",
                "create:cut_scoria_brick_stairs",
                "create:cut_scoria_brick_wall",
                "create:cut_scoria_bricks",
                "create:cut_scoria_stairs",
                "create:cut_scoria_wall",
                "create:layered_scoria",
                "create:polished_cut_scoria",
                "create:polished_cut_scoria_stairs",
                "create:polished_cut_scoria_wall",
                "create:scoria",
                "create:scoria_pillar",
                "create:small_scoria_brick_stairs",
                "create:small_scoria_brick_wall",
                "create:small_scoria_bricks",
            ],
        ),
        (
            "create:stone_types/tuff",
            &[
                "create:cut_tuff",
                "create:cut_tuff_brick_stairs",
                "create:cut_tuff_brick_wall",
                "create:cut_tuff_bricks",
                "create:cut_tuff_stairs",
                "create:cut_tuff_wall",
                "create:layered_tuff",
                "create:polished_cut_tuff",
                "create:polished_cut_tuff_stairs",
                "create:polished_cut_tuff_wall",
                "create:small_tuff_brick_stairs",
                "create:small_tuff_brick_wall",
                "create:small_tuff_bricks",
                "create:tuff_pillar",
                "minecraft:tuff",
            ],
        ),
        (
            "create:stone_types/veridium",
            &[
                "create:cut_veridium",
                "create:cut_veridium_brick_stairs",
                "create:cut_veridium_brick_wall",
                "create:cut_veridium_bricks",
                "create:cut_veridium_stairs",
                "create:cut_veridium_wall",
                "create:layered_veridium",
                "create:polished_cut_veridium",
                "create:polished_cut_veridium_stairs",
                "create:polished_cut_veridium_wall",
                "create:small_veridium_brick_stairs",
                "create:small_veridium_brick_wall",
                "create:small_veridium_bricks",
                "create:veridium",
                "create:veridium_pillar",
            ],
        ),
        (
            "create:table_cloths",
            &[
                "create:andesite_table_cloth",
                "create:black_table_cloth",
                "create:blue_table_cloth",
                "create:brass_table_cloth",
                "create:brown_table_cloth",
                "create:copper_table_cloth",
                "create:cyan_table_cloth",
                "create:gray_table_cloth",
                "create:green_table_cloth",
                "create:light_blue_table_cloth",
                "create:light_gray_table_cloth",
                "create:lime_table_cloth",
                "create:magenta_table_cloth",
                "create:orange_table_cloth",
                "create:pink_table_cloth",
                "create:purple_table_cloth",
                "create:red_table_cloth",
                "create:white_table_cloth",
                "create:yellow_table_cloth",
            ],
        ),
        (
            "create:toolboxes",
            &[
                "create:black_toolbox",
                "create:blue_toolbox",
                "create:brown_toolbox",
                "create:cyan_toolbox",
                "create:gray_toolbox",
                "create:green_toolbox",
                "create:light_blue_toolbox",
                "create:light_gray_toolbox",
                "create:lime_toolbox",
                "create:magenta_toolbox",
                "create:orange_toolbox",
                "create:pink_toolbox",
                "create:purple_toolbox",
                "create:red_toolbox",
                "create:white_toolbox",
                "create:yellow_toolbox",
            ],
        ),
        ("create:tracks", &["create:track"]),
        (
            "create:upgrade_aquatic/coral",
            &[
                "upgrade_aquatic:acan_coral",
                "upgrade_aquatic:branch_coral",
                "upgrade_aquatic:chrome_coral",
                "upgrade_aquatic:finger_coral",
                "upgrade_aquatic:moss_coral",
                "upgrade_aquatic:petal_coral",
                "upgrade_aquatic:pillow_coral",
                "upgrade_aquatic:rock_coral",
                "upgrade_aquatic:silk_coral",
                "upgrade_aquatic:star_coral",
            ],
        ),
        (
            "create:upright_on_belt",
            &[
                "atmospheric:aloe_gel_bottle",
                "atmospheric:orange_pudding",
                "atmospheric:orange_sorbet",
                "atmospheric:passion_fruit_sorbet",
                "autumnity:foul_soup",
                "autumnity:sap_bottle",
                "autumnity:syrup_bottle",
                "biomeswevegone:allium_oddion_soup",
                "biomeswevegone:aloe_vera_juice",
                "biomeswevegone:blue_glow_bottle",
                "biomeswevegone:green_glow_bottle",
                "biomeswevegone:red_glow_bottle",
                "biomeswevegone:white_puffball_stew",
                "biomeswevegone:yellow_glow_bottle",
                "create:blaze_cake",
                "create:blaze_cake_base",
                "create:builders_tea",
                "create:creative_blaze_cake",
                "minecraft:beetroot_soup",
                "minecraft:bowl",
                "minecraft:cake",
                "minecraft:glass_bottle",
                "minecraft:honey_bottle",
                "minecraft:lingering_potion",
                "minecraft:mushroom_stew",
                "minecraft:potion",
                "minecraft:rabbit_stew",
                "minecraft:splash_potion",
                "minecraft:suspicious_stew",
                "silentgems:cup_of_coffee",
                "silentgems:fishy_stew",
                "silentgems:meaty_stew",
                "silentgems:uncooked_fishy_stew",
                "silentgems:uncooked_meaty_stew",
            ],
        ),
        (
            "create:valve_handles",
            &[
                "create:black_valve_handle",
                "create:blue_valve_handle",
                "create:brown_valve_handle",
                "create:copper_valve_handle",
                "create:cyan_valve_handle",
                "create:gray_valve_handle",
                "create:green_valve_handle",
                "create:light_blue_valve_handle",
                "create:light_gray_valve_handle",
                "create:lime_valve_handle",
                "create:magenta_valve_handle",
                "create:orange_valve_handle",
                "create:pink_valve_handle",
                "create:purple_valve_handle",
                "create:red_valve_handle",
                "create:white_valve_handle",
                "create:yellow_valve_handle",
            ],
        ),
        ("curios:head", &["create:goggles"]),
        (
            "minecraft:axes",
            &[
                "minecraft:diamond_axe",
                "minecraft:golden_axe",
                "minecraft:iron_axe",
                "minecraft:netherite_axe",
                "minecraft:stone_axe",
                "minecraft:wooden_axe",
            ],
        ),
        (
            "minecraft:beacon_payment_items",
            &[
                "create:andesite_alloy",
                "create:brass_ingot",
                "create:zinc_ingot",
            ],
        ),
        (
            "minecraft:chest_armor",
            &[
                "create:cardboard_chestplate",
                "create:copper_backtank",
                "create:netherite_backtank",
            ],
        ),
        ("minecraft:coals", &["minecraft:charcoal", "minecraft:coal"]),
        (
            "minecraft:doors",
            &[
                "create:andesite_door",
                "create:brass_door",
                "create:copper_door",
                "create:framed_glass_door",
                "create:train_door",
            ],
        ),
        ("minecraft:enchantable/bow", &["create:potato_cannon"]),
        (
            "minecraft:enchantable/durability",
            &[
                "create:extendo_grip",
                "create:potato_cannon",
                "create:red_sand_paper",
                "create:sand_paper",
                "create:super_glue",
            ],
        ),
        (
            "minecraft:foot_armor",
            &[
                "create:cardboard_boots",
                "create:copper_diving_boots",
                "create:netherite_diving_boots",
            ],
        ),
        (
            "minecraft:head_armor",
            &[
                "create:cardboard_helmet",
                "create:copper_diving_helmet",
                "create:netherite_diving_helmet",
            ],
        ),
        (
            "minecraft:lava",
            &["minecraft:flowing_lava", "minecraft:lava"],
        ),
        ("minecraft:leg_armor", &["create:cardboard_leggings"]),
        (
            "minecraft:piglin_loved",
            &["create:crushed_raw_gold", "create:golden_sheet"],
        ),
        (
            "minecraft:planks",
            &[
                "minecraft:acacia_planks",
                "minecraft:bamboo_planks",
                "minecraft:birch_planks",
                "minecraft:cherry_planks",
                "minecraft:crimson_planks",
                "minecraft:dark_oak_planks",
                "minecraft:jungle_planks",
                "minecraft:mangrove_planks",
                "minecraft:oak_planks",
                "minecraft:pale_oak_planks",
                "minecraft:spruce_planks",
                "minecraft:warped_planks",
            ],
        ),
        (
            "minecraft:slabs",
            &[
                "create:cut_andesite_brick_slab",
                "create:cut_andesite_slab",
                "create:cut_asurine_brick_slab",
                "create:cut_asurine_slab",
                "create:cut_calcite_brick_slab",
                "create:cut_calcite_slab",
                "create:cut_crimsite_brick_slab",
                "create:cut_crimsite_slab",
                "create:cut_deepslate_brick_slab",
                "create:cut_deepslate_slab",
                "create:cut_diorite_brick_slab",
                "create:cut_diorite_slab",
                "create:cut_dripstone_brick_slab",
                "create:cut_dripstone_slab",
                "create:cut_granite_brick_slab",
                "create:cut_granite_slab",
                "create:cut_limestone_brick_slab",
                "create:cut_limestone_slab",
                "create:cut_ochrum_brick_slab",
                "create:cut_ochrum_slab",
                "create:cut_scorchia_brick_slab",
                "create:cut_scorchia_slab",
                "create:cut_scoria_brick_slab",
                "create:cut_scoria_slab",
                "create:cut_tuff_brick_slab",
                "create:cut_tuff_slab",
                "create:cut_veridium_brick_slab",
                "create:cut_veridium_slab",
                "create:polished_cut_andesite_slab",
                "create:polished_cut_asurine_slab",
                "create:polished_cut_calcite_slab",
                "create:polished_cut_crimsite_slab",
                "create:polished_cut_deepslate_slab",
                "create:polished_cut_diorite_slab",
                "create:polished_cut_dripstone_slab",
                "create:polished_cut_granite_slab",
                "create:polished_cut_limestone_slab",
                "create:polished_cut_ochrum_slab",
                "create:polished_cut_scorchia_slab",
                "create:polished_cut_scoria_slab",
                "create:polished_cut_tuff_slab",
                "create:polished_cut_veridium_slab",
                "create:small_andesite_brick_slab",
                "create:small_asurine_brick_slab",
                "create:small_calcite_brick_slab",
                "create:small_crimsite_brick_slab",
                "create:small_deepslate_brick_slab",
                "create:small_diorite_brick_slab",
                "create:small_dripstone_brick_slab",
                "create:small_granite_brick_slab",
                "create:small_limestone_brick_slab",
                "create:small_ochrum_brick_slab",
                "create:small_scorchia_brick_slab",
                "create:small_scoria_brick_slab",
                "create:small_tuff_brick_slab",
                "create:small_veridium_brick_slab",
            ],
        ),
        (
            "minecraft:stairs",
            &[
                "create:cut_andesite_brick_stairs",
                "create:cut_andesite_stairs",
                "create:cut_asurine_brick_stairs",
                "create:cut_asurine_stairs",
                "create:cut_calcite_brick_stairs",
                "create:cut_calcite_stairs",
                "create:cut_crimsite_brick_stairs",
                "create:cut_crimsite_stairs",
                "create:cut_deepslate_brick_stairs",
                "create:cut_deepslate_stairs",
                "create:cut_diorite_brick_stairs",
                "create:cut_diorite_stairs",
                "create:cut_dripstone_brick_stairs",
                "create:cut_dripstone_stairs",
                "create:cut_granite_brick_stairs",
                "create:cut_granite_stairs",
                "create:cut_limestone_brick_stairs",
                "create:cut_limestone_stairs",
                "create:cut_ochrum_brick_stairs",
                "create:cut_ochrum_stairs",
                "create:cut_scorchia_brick_stairs",
                "create:cut_scorchia_stairs",
                "create:cut_scoria_brick_stairs",
                "create:cut_scoria_stairs",
                "create:cut_tuff_brick_stairs",
                "create:cut_tuff_stairs",
                "create:cut_veridium_brick_stairs",
                "create:cut_veridium_stairs",
                "create:polished_cut_andesite_stairs",
                "create:polished_cut_asurine_stairs",
                "create:polished_cut_calcite_stairs",
                "create:polished_cut_crimsite_stairs",
                "create:polished_cut_deepslate_stairs",
                "create:polished_cut_diorite_stairs",
                "create:polished_cut_dripstone_stairs",
                "create:polished_cut_granite_stairs",
                "create:polished_cut_limestone_stairs",
                "create:polished_cut_ochrum_stairs",
                "create:polished_cut_scorchia_stairs",
                "create:polished_cut_scoria_stairs",
                "create:polished_cut_tuff_stairs",
                "create:polished_cut_veridium_stairs",
                "create:small_andesite_brick_stairs",
                "create:small_asurine_brick_stairs",
                "create:small_calcite_brick_stairs",
                "create:small_crimsite_brick_stairs",
                "create:small_deepslate_brick_stairs",
                "create:small_diorite_brick_stairs",
                "create:small_dripstone_brick_stairs",
                "create:small_granite_brick_stairs",
                "create:small_limestone_brick_stairs",
                "create:small_ochrum_brick_stairs",
                "create:small_scorchia_brick_stairs",
                "create:small_scoria_brick_stairs",
                "create:small_tuff_brick_stairs",
                "create:small_veridium_brick_stairs",
            ],
        ),
        (
            "minecraft:trapdoors",
            &["create:framed_glass_trapdoor", "create:train_trapdoor"],
        ),
        ("minecraft:trimmable_armor", &[]),
        (
            "minecraft:walls",
            &[
                "create:cut_andesite_brick_wall",
                "create:cut_andesite_wall",
                "create:cut_asurine_brick_wall",
                "create:cut_asurine_wall",
                "create:cut_calcite_brick_wall",
                "create:cut_calcite_wall",
                "create:cut_crimsite_brick_wall",
                "create:cut_crimsite_wall",
                "create:cut_deepslate_brick_wall",
                "create:cut_deepslate_wall",
                "create:cut_diorite_brick_wall",
                "create:cut_diorite_wall",
                "create:cut_dripstone_brick_wall",
                "create:cut_dripstone_wall",
                "create:cut_granite_brick_wall",
                "create:cut_granite_wall",
                "create:cut_limestone_brick_wall",
                "create:cut_limestone_wall",
                "create:cut_ochrum_brick_wall",
                "create:cut_ochrum_wall",
                "create:cut_scorchia_brick_wall",
                "create:cut_scorchia_wall",
                "create:cut_scoria_brick_wall",
                "create:cut_scoria_wall",
                "create:cut_tuff_brick_wall",
                "create:cut_tuff_wall",
                "create:cut_veridium_brick_wall",
                "create:cut_veridium_wall",
                "create:polished_cut_andesite_wall",
                "create:polished_cut_asurine_wall",
                "create:polished_cut_calcite_wall",
                "create:polished_cut_crimsite_wall",
                "create:polished_cut_deepslate_wall",
                "create:polished_cut_diorite_wall",
                "create:polished_cut_dripstone_wall",
                "create:polished_cut_granite_wall",
                "create:polished_cut_limestone_wall",
                "create:polished_cut_ochrum_wall",
                "create:polished_cut_scorchia_wall",
                "create:polished_cut_scoria_wall",
                "create:polished_cut_tuff_wall",
                "create:polished_cut_veridium_wall",
                "create:small_andesite_brick_wall",
                "create:small_asurine_brick_wall",
                "create:small_calcite_brick_wall",
                "create:small_crimsite_brick_wall",
                "create:small_deepslate_brick_wall",
                "create:small_diorite_brick_wall",
                "create:small_dripstone_brick_wall",
                "create:small_granite_brick_wall",
                "create:small_limestone_brick_wall",
                "create:small_ochrum_brick_wall",
                "create:small_scorchia_brick_wall",
                "create:small_scoria_brick_wall",
                "create:small_tuff_brick_wall",
                "create:small_veridium_brick_wall",
            ],
        ),
        (
            "minecraft:water",
            &["minecraft:flowing_water", "minecraft:water"],
        ),
        (
            "minecraft:wool",
            &[
                "minecraft:black_wool",
                "minecraft:blue_wool",
                "minecraft:brown_wool",
                "minecraft:cyan_wool",
                "minecraft:gray_wool",
                "minecraft:green_wool",
                "minecraft:light_blue_wool",
                "minecraft:light_gray_wool",
                "minecraft:lime_wool",
                "minecraft:magenta_wool",
                "minecraft:orange_wool",
                "minecraft:pink_wool",
                "minecraft:purple_wool",
                "minecraft:red_wool",
                "minecraft:white_wool",
                "minecraft:yellow_wool",
            ],
        ),
    ]
}

}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Heat {
    None,
    Heated,
    Superheated,
}

#[derive(Clone, Copy, Debug)]
pub struct Ingredient {
    /// An item/fluid ID, or #tag.
    pub matcher: &'static str,
    pub count: u32,
    pub fluid: bool,
    pub consume: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Product {
    pub id: &'static str,
    pub count: u32,
    pub fluid: bool,
    pub chance: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub id: &'static str,
    pub station: &'static str,
    pub ingredients: &'static [Ingredient],
    pub products: &'static [Product],
    pub work: u32,
    pub heat: Heat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stock {
    pub items: BTreeMap<String, u32>,
    pub fluids: BTreeMap<String, u32>,
    pub slots: usize,
    pub tank_capacity: u32,
}
impl Default for Stock {
    fn default() -> Self {
        Self {
            items: BTreeMap::new(),
            fluids: BTreeMap::new(),
            slots: 9,
            tank_capacity: 4000,
        }
    }
}
impl Stock {
    /// Laboratory stacks use 64 items per slot. No game stack-size claims.
    pub fn insert(&mut self, id: &str, amount: u32, fluid: bool) -> Result<(), String> {
        if amount == 0 || !valid_id(id) {
            return Err("Expected a namespaced resource and a positive amount".into());
        }
        let mut trial = self.clone();
        let map = if fluid {
            &mut trial.fluids
        } else {
            &mut trial.items
        };
        let value = map.entry(id.to_string()).or_default();
        *value = value.checked_add(amount).ok_or("Resource count overflow")?;
        if trial.used_slots() > trial.slots
            || trial.fluids.values().map(|n| u64::from(*n)).sum::<u64>()
                > u64::from(trial.tank_capacity)
        {
            return Err("Inventory or tank is full".into());
        }
        *self = trial;
        Ok(())
    }
    pub fn used_slots(&self) -> usize {
        self.items
            .values()
            .map(|n| u64::from(*n).div_ceil(64) as usize)
            .sum()
    }
    pub fn summary(&self) -> String {
        let mut entries: Vec<String> = self
            .items
            .iter()
            .map(|(id, n)| format!("{id} x{n}"))
            .collect();
        entries.extend(self.fluids.iter().map(|(id, n)| format!("{id} {n}mB")));
        if entries.is_empty() {
            "empty".into()
        } else {
            entries.join(", ")
        }
    }
}
pub fn valid_id(id: &str) -> bool {
    id.len() <= 128
        && id.split_once(':').is_some_and(|(ns, path)| {
            !ns.is_empty()
                && !path.is_empty()
                && ns
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
                && path
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_./-".contains(&c))
        })
}
fn matches(matcher: &str, id: &str) -> bool {
    match matcher.strip_prefix('#') {
        None => matcher == id,
        Some(tag) => catalog::tags()
            .iter()
            .any(|(t, ids)| *t == tag && ids.contains(&id)),
    }
}

/// Finds an allocation without consuming anything in the live inventory.
/// Backtracking handles overlapping tags and repeated ingredient slots.
fn reserve(stock: &Stock, recipe: &Recipe) -> Option<Stock> {
    fn step(
        stock: Stock,
        list: &[Ingredient],
        i: usize,
        kept: &mut Vec<(String, u32, bool)>,
        budget: &mut usize,
    ) -> Option<Stock> {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if i == list.len() {
            let mut stock = stock;
            for (id, count, fluid) in kept.iter() {
                let map = if *fluid {
                    &mut stock.fluids
                } else {
                    &mut stock.items
                };
                *map.entry(id.clone()).or_default() += *count;
            }
            return Some(stock);
        }
        let ing = list[i];
        let map = if ing.fluid {
            &stock.fluids
        } else {
            &stock.items
        };
        let options: Vec<String> = map
            .iter()
            .filter(|(id, n)| **n >= ing.count && matches(ing.matcher, id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in options {
            let mut next = stock.clone();
            let map = if ing.fluid {
                &mut next.fluids
            } else {
                &mut next.items
            };
            *map.get_mut(&id).unwrap() -= ing.count;
            map.retain(|_, n| *n != 0);
            if !ing.consume {
                kept.push((id, ing.count, ing.fluid));
            }
            if let Some(done) = step(next, list, i + 1, kept, budget) {
                return Some(done);
            }
            if !ing.consume {
                kept.pop();
            }
        }
        None
    }
    let mut list = recipe.ingredients.to_vec();
    list.sort_by_key(|i| {
        let map = if i.fluid { &stock.fluids } else { &stock.items };
        map.iter()
            .filter(|(id, n)| **n >= i.count && matches(i.matcher, id))
            .count()
    });
    step(stock.clone(), &list, 0, &mut Vec::new(), &mut 4096)
}

#[derive(Clone, Debug)]
pub struct Machine {
    pub station: String,
    pub input: Stock,
    pub output: Stock,
    pub progress: f32,
    pub heat: Heat,
    pub selected: Option<String>,
    /// Persisted, deterministic laboratory RNG; not Java's random stream.
    pub rng: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickResult {
    Idle,
    NoPower,
    TooSlow,
    Cold,
    Working,
    OutputBlocked,
    Completed,
}

fn roll(state: &mut u64) -> f64 {
    if *state == 0 {
        *state = 0x4d595df4d0f33173;
    }
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 11) as f64) / ((1_u64 << 53) as f64)
}
impl Machine {
    pub fn new(station: &str) -> Self {
        Self {
            station: station.into(),
            input: Stock::default(),
            output: Stock::default(),
            progress: 0.0,
            heat: Heat::None,
            selected: None,
            rng: 0x4d595df4d0f33173,
        }
    }
    pub fn tick(&mut self, rpm: f32) -> TickResult {
        self.tick_with(rpm, catalog::recipes())
    }
    pub fn tick_with(&mut self, rpm: f32, recipes: &[Recipe]) -> TickResult {
        if !rpm.is_finite() || rpm == 0.0 {
            return TickResult::NoPower;
        }
        if self.station == "mixing" && rpm.abs() < 32.0 {
            return TickResult::TooSlow;
        }
        let selected = self.selected.as_deref().and_then(|id| {
            recipes.iter().find(|r| {
                r.id == id && r.station == self.station && reserve(&self.input, r).is_some()
            })
        });
        let recipe = selected.or_else(|| {
            recipes
                .iter()
                .find(|r| r.station == self.station && reserve(&self.input, r).is_some())
        });
        let Some(recipe) = recipe else {
            self.progress = 0.0;
            self.selected = None;
            return TickResult::Idle;
        };
        if self.selected.as_deref() != Some(recipe.id) {
            self.progress = 0.0;
            self.selected = Some(recipe.id.into());
        }
        if self.heat < recipe.heat {
            return TickResult::Cold;
        }
        self.progress =
            (self.progress + (rpm.abs() / 32.0).clamp(0.03125, 8.0)).min(recipe.work.max(1) as f32);
        if self.progress < recipe.work.max(1) as f32 {
            return TickResult::Working;
        }
        // Reserve the worst-case outputs before advancing RNG or consuming
        // inputs. A blocked retry cannot reroll for a more favourable result.
        let mut worst = self.output.clone();
        for p in recipe.products {
            if p.chance > 0.0 && worst.insert(p.id, p.count, p.fluid).is_err() {
                return TickResult::OutputBlocked;
            }
        }
        let next_input = reserve(&self.input, recipe).expect("recipe was matched above");
        let mut next_output = self.output.clone();
        let mut rng = self.rng;
        for p in recipe.products {
            // Create rolls each member of a chance stack independently.
            let count = if p.chance >= 1.0 {
                p.count
            } else {
                (0..p.count).filter(|_| roll(&mut rng) < p.chance).count() as u32
            };
            if count > 0 {
                next_output
                    .insert(p.id, count, p.fluid)
                    .expect("worst-case output reserved");
            }
        }
        self.input = next_input;
        self.output = next_output;
        self.rng = rng;
        self.progress = 0.0;
        TickResult::Completed
    }
    pub fn encode(&self) -> String {
        let mut out = format!(
            "CHLAB1 {} {} {} {}\n",
            self.station, self.progress, self.heat as u8, self.rng
        );
        if let Some(id) = &self.selected {
            out.push_str(&format!("recipe {id}\n"));
        }
        for (label, stock) in [("in", &self.input), ("out", &self.output)] {
            for (id, n) in &stock.items {
                out.push_str(&format!("{label} item {id} {n}\n"));
            }
            for (id, n) in &stock.fluids {
                out.push_str(&format!("{label} fluid {id} {n}\n"));
            }
        }
        out
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        let ["CHLAB1", station, progress, heat, rng] = header.as_slice() else {
            return Err("Unsupported laboratory save".into());
        };
        if !catalog::recipes().iter().any(|r| r.station == *station) {
            return Err("Unknown station".into());
        }
        let mut m = Self::new(station);
        m.progress = progress.parse().map_err(|_| "Invalid progress")?;
        if !m.progress.is_finite() || !(0.0..=100000.0).contains(&m.progress) {
            return Err("Invalid progress".into());
        }
        m.heat = match *heat {
            "0" => Heat::None,
            "1" => Heat::Heated,
            "2" => Heat::Superheated,
            _ => return Err("Invalid heat".into()),
        };
        m.rng = rng.parse().map_err(|_| "Invalid random state")?;
        for line in lines {
            let f: Vec<_> = line.split_whitespace().collect();
            match f.as_slice() {
                ["recipe", id]
                    if m.selected.is_none()
                        && catalog::recipes()
                            .iter()
                            .any(|r| r.id == *id && r.station == *station) =>
                {
                    m.selected = Some((*id).into())
                }
                [side, kind, id, n]
                    if matches!(*side, "in" | "out") && matches!(*kind, "item" | "fluid") =>
                {
                    let stock = if *side == "in" {
                        &mut m.input
                    } else {
                        &mut m.output
                    };
                    stock.insert(
                        id,
                        n.parse().map_err(|_| "Invalid resource amount")?,
                        *kind == "fluid",
                    )?;
                }
                _ => return Err("Invalid laboratory save row".into()),
            }
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
use super::*;
fn recipe(id: &str) -> &'static Recipe {
    catalog::recipes().iter().find(|r| r.id == id).unwrap()
}
fn finish(m: &mut Machine, rpm: f32) -> TickResult {
    for _ in 0..10000 {
        let r = m.tick(rpm);
        if !matches!(r, TickResult::Working) {
            return r;
        }
    }
    panic!("recipe never finished")
}
#[test]
fn actual_iron_pressing_consumes_exactly_one_ingot() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 2, false).unwrap();
    assert_eq!(finish(&mut m, 32.0), TickResult::Completed);
    assert_eq!(m.input.items["minecraft:iron_ingot"], 1);
    assert_eq!(m.output.items["create:iron_sheet"], 1);
}
#[test]
fn no_power_preserves_inputs_and_work() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick(32.0), TickResult::Working);
    let before = m.encode();
    for rpm in [0.0, f32::NAN, f32::INFINITY] {
        assert_eq!(m.tick(rpm), TickResult::NoPower);
        assert_eq!(m.encode(), before);
    }
}
#[test]
fn blocked_output_cannot_consume_inputs_or_reroll() {
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    m.output.slots = 0;
    assert_eq!(finish(&mut m, 256.0), TickResult::OutputBlocked);
    let saved = m.encode();
    for _ in 0..20 {
        assert_eq!(m.tick(256.0), TickResult::OutputBlocked);
        assert_eq!(m.encode(), saved);
    }
    m.output.slots = 9;
    assert_eq!(m.tick(256.0), TickResult::Completed);
    assert!(m.input.items.is_empty());
}
#[test]
fn stock_insertion_is_atomic_and_checks_stack_and_fluid_limits() {
    let mut s = Stock {
        slots: 1,
        ..Stock::default()
    };
    s.insert("minecraft:iron_ingot", 64, false).unwrap();
    let old = s.clone();
    assert!(s.insert("minecraft:gold_ingot", 1, false).is_err());
    assert_eq!(s, old);
    assert!(s.insert("minecraft:water", 4001, true).is_err());
    assert_eq!(s, old);
    assert!(s.insert("bad id", 1, false).is_err());
    assert!(s.insert("minecraft:water", 0, true).is_err());
}
#[test]
fn hot_mixing_requires_heat_and_at_least_32_rpm() {
    let r = recipe("create:mixing/brass_ingot");
    let mut m = Machine::new("mixing");
    for i in r.ingredients {
        let id = i
            .matcher
            .strip_prefix('#')
            .and_then(|tag| catalog::tags().iter().find(|(t, _)| *t == tag))
            .map(|(_, ids)| ids[0])
            .unwrap_or(i.matcher);
        m.input.insert(id, i.count, i.fluid).unwrap();
    }
    assert_eq!(m.tick(16.0), TickResult::TooSlow);
    assert_eq!(m.tick(32.0), TickResult::Cold);
    m.heat = Heat::Superheated;
    assert_eq!(finish(&mut m, 64.0), TickResult::Completed);
}
#[test]
fn fluid_filling_reserves_both_the_bottle_and_fluid() {
    let mut m = Machine::new("filling");
    m.input.insert("minecraft:glass_bottle", 1, false).unwrap();
    m.input.insert("create:honey", 250, true).unwrap();
    assert_eq!(finish(&mut m, 32.0), TickResult::Completed);
    assert!(m.input.items.is_empty());
    assert!(m.input.fluids.is_empty());
    assert_eq!(m.output.items["minecraft:honey_bottle"], 1);
}
#[test]
fn save_reload_preserves_recipe_progress_and_random_state() {
    let mut m = Machine::new("milling");
    m.input.insert("minecraft:wheat", 5, false).unwrap();
    for _ in 0..7 {
        m.tick(32.0);
    }
    let mut restored = Machine::decode(&m.encode()).unwrap();
    assert_eq!(m.encode(), restored.encode());
    for _ in 0..300 {
        assert_eq!(m.tick(32.0), restored.tick(32.0));
    }
    assert_eq!(m.encode(), restored.encode());
}
#[test]
fn malformed_saves_fail_closed() {
    for text in [
        "garbage",
        "CHLAB1 pressing NaN 0 1\n",
        "CHLAB1 pressing -1 0 1\n",
        "CHLAB1 pressing 0 4 1\n",
        "CHLAB1 pressing 0 0 1\nin item minecraft:iron_ingot 999999\n",
        "CHLAB1 pressing 0 0 1\nrecipe missing\n",
    ] {
        assert!(Machine::decode(text).is_err(), "{text}");
    }
}
#[test]
fn repeated_ingredients_cannot_share_a_single_item() {
    static R: Recipe = Recipe {
        id: "test:repeat",
        station: "pressing",
        ingredients: &[
            Ingredient {
                matcher: "minecraft:iron_ingot",
                count: 1,
                fluid: false,
                consume: true,
            },
            Ingredient {
                matcher: "minecraft:iron_ingot",
                count: 1,
                fluid: false,
                consume: true,
            },
        ],
        products: &[Product {
            id: "create:iron_sheet",
            count: 1,
            fluid: false,
            chance: 1.0,
        }],
        work: 1,
        heat: Heat::None,
    };
    let mut m = Machine::new("pressing");
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Idle);
    m.input.insert("minecraft:iron_ingot", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Completed);
    assert!(m.input.items.is_empty());
}
#[test]
fn retained_tools_are_reserved_and_returned_after_a_craft() {
    static R: Recipe = Recipe {
        id: "test:tool",
        station: "deploying",
        ingredients: &[
            Ingredient {
                matcher: "minecraft:oak_log",
                count: 1,
                fluid: false,
                consume: true,
            },
            Ingredient {
                matcher: "minecraft:iron_axe",
                count: 1,
                fluid: false,
                consume: false,
            },
        ],
        products: &[Product {
            id: "minecraft:stripped_oak_log",
            count: 1,
            fluid: false,
            chance: 1.0,
        }],
        work: 1,
        heat: Heat::None,
    };
    let mut m = Machine::new("deploying");
    m.input.insert("minecraft:oak_log", 1, false).unwrap();
    m.input.insert("minecraft:iron_axe", 1, false).unwrap();
    assert_eq!(m.tick_with(32.0, &[R]), TickResult::Completed);
    assert_eq!(m.input.items["minecraft:iron_axe"], 1);
    assert!(!m.input.items.contains_key("minecraft:oak_log"));
}
#[test]
fn every_imported_recipe_can_complete_with_valid_inputs() {
    for r in catalog::recipes() {
        let mut m = Machine::new(r.station);
        m.heat = Heat::Superheated;
        for i in r.ingredients {
            let id = if let Some(tag) = i.matcher.strip_prefix('#') {
                catalog::tags().iter().find(|(t, _)| *t == tag).unwrap().1[0]
            } else {
                i.matcher
            };
            m.input
                .insert(id, i.count, i.fluid)
                .unwrap_or_else(|e| panic!("{}: {e}", r.id));
        }
        // Select this recipe explicitly: the same ingredients may have several recipes.
        m.selected = Some(r.id.into());
        assert_eq!(finish(&mut m, 256.0), TickResult::Completed, "{}", r.id);
    }
}

}

}
#[allow(dead_code)]
pub mod machines {
// Native machine transactions over the host's real inventory slots.
// Saved state contains progress and fluids, never a second copy of items.
pub use super::transmission as kinetics;
pub use super::recipes::{Heat, TickResult};
use super::recipes::{Machine, Stock};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const TANK_CAPACITY: u32 = 8000;
pub const INPUT: std::ops::Range<usize> = 0..9;
pub const OUTPUT: std::ops::Range<usize> = 9..18;

/// Components are canonical, opaque host SNBT. Unknown components survive moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    pub components: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MachineState {
    pub version: u32,
    pub progress: f32,
    pub recipe: Option<String>,
    pub rng: u64,
    pub fluid_in: BTreeMap<String, u32>,
    pub fluid_out: BTreeMap<String, u32>,
    pub direction: i8,
    pub engaged: bool,
    pub crank_until: u64,
    pub burn_until: u64,
    pub superheat_until: u64,
    pub rpm: f32,
    pub angle: f32,
    pub speed_since: u64,
    pub capacity: f32,
    pub stress: f32,
    pub status: String,
    pub motor_rpm: f32,
}
impl Default for MachineState {
    fn default() -> Self {
        Self {
            version: 1,
            progress: 0.,
            recipe: None,
            rng: 0x4d595df4d0f33173,
            fluid_in: BTreeMap::new(),
            fluid_out: BTreeMap::new(),
            direction: 1,
            engaged: true,
            crank_until: 0,
            burn_until: 0,
            superheat_until: 0,
            rpm: 0.,
            angle: 0.,
            speed_since: 0,
            capacity: 0.,
            stress: 0.,
            status: String::new(),
            motor_rpm: 16.0,
        }
    }
}
impl MachineState {
    pub fn decode(data: &str) -> Result<Self, String> {
        if data.is_empty() {
            return Ok(Self::default());
        }
        if data.len() > 65536 {
            return Err("Machine data exceeds limit".into());
        }
        let state: Self = serde_json::from_str(data).map_err(|e| e.to_string())?;
        if state.version != 1
            || !state.progress.is_finite()
            || !(0.0..=100000.).contains(&state.progress)
            || !state.motor_rpm.is_finite()
            || !(0.0..=256.0).contains(&state.motor_rpm)
            || !state.rpm.is_finite()
            || state.rpm.abs() > 256.
            || !state.angle.is_finite()
            || !state.capacity.is_finite()
            || state.capacity < 0.
            || !state.stress.is_finite()
            || state.stress < 0.
            || ![-1, 1].contains(&state.direction)
            || !valid_tank(&state.fluid_in)
            || !valid_tank(&state.fluid_out)
            || state
                .recipe
                .as_deref()
                .is_some_and(|id| !super::recipes::valid_id(id))
        {
            return Err("Invalid machine data".into());
        }
        Ok(state)
    }
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("finite native machine state")
    }
    pub fn render_angle(&self, tick: u64, partial: f32) -> f32 {
        (f64::from(self.angle)
            + (tick.saturating_sub(self.speed_since) as f64 + f64::from(partial.clamp(0., 1.)))
                * f64::from(self.rpm)
                * 0.3)
            .rem_euclid(360.) as f32
    }
    pub fn set_speed(&mut self, rpm: f32, tick: u64) {
        let rpm = if rpm.is_finite() {
            rpm.clamp(-256., 256.)
        } else {
            0.
        };
        if self.rpm != rpm {
            self.angle = self.render_angle(tick, 0.);
            self.speed_since = tick;
            self.rpm = rpm;
        }
    }
    pub fn insert_fluid(&mut self, id: &str, amount: u32) -> bool {
        if amount == 0
            || !super::recipes::valid_id(id)
            || tank_total(&self.fluid_in) + u64::from(amount) > u64::from(TANK_CAPACITY)
        {
            return false;
        }
        *self.fluid_in.entry(id.into()).or_default() += amount;
        true
    }
    /// Output has priority, then the input tank. Extraction is all-or-nothing.
    pub fn extract_fluid(&mut self, id: &str, amount: u32) -> bool {
        if amount == 0 {
            return false;
        }
        for tank in [&mut self.fluid_out, &mut self.fluid_in] {
            if tank.get(id).copied().unwrap_or(0) >= amount {
                let n = tank.get_mut(id).unwrap();
                *n -= amount;
                tank.retain(|_, n| *n > 0);
                return true;
            }
        }
        false
    }
}
fn tank_total(tank: &BTreeMap<String, u32>) -> u64 {
    tank.values().map(|n| u64::from(*n)).sum()
}
fn valid_tank(tank: &BTreeMap<String, u32>) -> bool {
    tank_total(tank) <= u64::from(TANK_CAPACITY)
        && tank
            .iter()
            .all(|(id, n)| *n > 0 && super::recipes::valid_id(id))
}
#[derive(Clone, Debug)]
pub struct InventoryPlan {
    pub before: Vec<Option<Stack>>,
    pub after: Vec<Option<Stack>>,
}
impl InventoryPlan {
    pub fn commit(self, live: &mut Vec<Option<Stack>>) -> bool {
        if *live != self.before {
            return false;
        }
        *live = self.after;
        true
    }
}
fn aggregate(slots: &[Option<Stack>], range: std::ops::Range<usize>) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();
    for s in slots[range]
        .iter()
        .flatten()
        .filter(|s| s.components.is_empty())
    {
        *map.entry(s.id.clone()).or_default() += s.count;
    }
    map
}
fn insert(slots: &mut [Option<Stack>], stack: &Stack, limit: impl Fn(&str) -> u32) -> bool {
    let max = limit(&stack.id).max(1);
    let mut left = stack.count;
    for current in slots.iter_mut().flatten() {
        if current.id == stack.id && current.components == stack.components {
            let n = left.min(max.saturating_sub(current.count));
            current.count += n;
            left -= n;
        }
    }
    for current in slots {
        if left == 0 {
            break;
        }
        if current.is_none() {
            let n = left.min(max);
            *current = Some(Stack {
                count: n,
                ..stack.clone()
            });
            left -= n;
        }
    }
    left == 0
}
/// Rebuild only the consumed plain inputs; componentful stacks are never recipe ingredients.
fn consume(slots: &mut [Option<Stack>], remaining: &BTreeMap<String, u32>) {
    let mut remaining = remaining.clone();
    for slot in slots {
        if let Some(s) = slot
            && s.components.is_empty()
        {
            let n = remaining.entry(s.id.clone()).or_default();
            let keep = s.count.min(*n);
            *n -= keep;
            if keep == 0 {
                *slot = None;
            } else {
                s.count = keep;
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Update {
    pub state: MachineState,
    pub inventory: InventoryPlan,
    pub result: TickResult,
}
/// Reserve real output space before consuming inputs or committing random state.
/// Exact Java durations, durability-based tools and component recipes remain unsupported.
pub fn process(
    state: &MachineState,
    slots: &[Option<Stack>],
    station: &str,
    rpm: f32,
    heat: Heat,
    limit: impl Fn(&str) -> u32,
) -> Result<Update, String> {
    process_with_catalog(
        state,
        slots,
        station,
        rpm,
        heat,
        super::recipes::catalog::recipes(),
        limit,
    )
}
pub fn process_with_catalog(
    state: &MachineState,
    slots: &[Option<Stack>],
    station: &str,
    rpm: f32,
    heat: Heat,
    recipes: &[super::recipes::Recipe],
    limit: impl Fn(&str) -> u32,
) -> Result<Update, String> {
    if slots.len() < 18 {
        return Err("Machine needs input and output slots".into());
    }
    let mut machine = Machine::new(station);
    machine.input = Stock {
        items: aggregate(slots, INPUT),
        fluids: state.fluid_in.clone(),
        slots: usize::MAX,
        tank_capacity: TANK_CAPACITY,
    };
    // The real output slots are reserved separately, using actual item stack limits.
    machine.output = Stock {
        items: BTreeMap::new(),
        fluids: state.fluid_out.clone(),
        slots: usize::MAX,
        tank_capacity: TANK_CAPACITY,
    };
    machine.progress = state.progress;
    machine.selected = state.recipe.clone();
    machine.rng = state.rng;
    machine.heat = heat;
    let mut result = machine.tick_with(rpm, recipes);
    let mut next = state.clone();
    let mut after = slots.to_vec();
    if result == TickResult::Completed {
        let recipe = recipes
            .iter()
            .find(|r| Some(r.id) == machine.selected.as_deref())
            .ok_or("Missing recipe")?;
        let mut worst = after[OUTPUT].to_vec();
        let space = recipe
            .products
            .iter()
            .filter(|p| !p.fluid && p.chance > 0.)
            .all(|p| {
                insert(
                    &mut worst,
                    &Stack {
                        id: p.id.into(),
                        count: p.count,
                        components: String::new(),
                    },
                    &limit,
                )
            });
        if !space {
            result = TickResult::OutputBlocked;
            machine.progress = recipe.work.max(1) as f32;
        } else {
            consume(&mut after[INPUT], &machine.input.items);
            for (id, count) in &machine.output.items {
                if !insert(
                    &mut after[OUTPUT],
                    &Stack {
                        id: id.clone(),
                        count: *count,
                        components: String::new(),
                    },
                    &limit,
                ) {
                    return Err("Output reservation mismatch".into());
                }
            }
            next.fluid_in = machine.input.fluids;
            next.fluid_out = machine.output.fluids;
            next.rng = machine.rng;
        }
    }
    next.progress = machine.progress;
    next.recipe = machine.selected;
    next.status = format!("{result:?}");
    Ok(Update {
        state: next,
        inventory: InventoryPlan {
            before: slots.to_vec(),
            after,
        },
        result,
    })
}
/// Two snapshots, committed together by the host only while both are unchanged.
pub fn transfer(
    source: &[Option<Stack>],
    destination: &[Option<Stack>],
    amount: u32,
    limit: impl Fn(&str) -> u32,
) -> Option<(InventoryPlan, InventoryPlan)> {
    if amount == 0 {
        return None;
    }
    for (i, stack) in source
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|s| (i, s)))
    {
        let mut item = stack.clone();
        item.count = item.count.min(amount);
        let mut target = destination.to_vec();
        if !insert(&mut target, &item, &limit) {
            continue;
        }
        let mut from = source.to_vec();
        let left = stack.count - item.count;
        from[i] = if left == 0 {
            None
        } else {
            Some(Stack {
                count: left,
                ..stack.clone()
            })
        };
        return Some((
            InventoryPlan {
                before: source.to_vec(),
                after: from,
            },
            InventoryPlan {
                before: destination.to_vec(),
                after: target,
            },
        ));
    }
    None
}
pub fn transfer_fluid(
    source: &MachineState,
    destination: &MachineState,
    budget: u32,
) -> Option<(MachineState, MachineState, u32)> {
    let room = u64::from(TANK_CAPACITY).checked_sub(tank_total(&destination.fluid_in))? as u32;
    let (id, count) = source
        .fluid_out
        .iter()
        .chain(source.fluid_in.iter())
        .find(|(_, n)| **n > 0)?;
    let amount = (*count).min(budget).min(room);
    if amount == 0 {
        return None;
    }
    let mut from = source.clone();
    let mut to = destination.clone();
    if !from.extract_fluid(id, amount) || !to.insert_fluid(id, amount) {
        return None;
    }
    Some((from, to, amount))
}
pub fn station(block: &str) -> Option<&'static str> {
    Some(match block.strip_prefix("create:")? {
        "millstone" => "milling",
        "mechanical_press" => "pressing",
        "mechanical_mixer" => "mixing",
        "mechanical_saw" => "cutting",
        "crushing_wheel" => "crushing",
        "deployer" => "deploying",
        "encased_fan" => "splashing",
        "spout" => "filling",
        "item_drain" => "emptying",
        _ => return None,
    })
}
pub fn container_size(block: &str) -> Option<usize> {
    if station(block).is_some() {
        return Some(27);
    }
    let path = block.strip_prefix("create:")?;
    if matches!(
        path,
        "basin"
            | "depot"
            | "belt"
            | "chute"
            | "smart_chute"
            | "andesite_funnel"
            | "brass_funnel"
            | "item_vault"
            | "mechanical_crafter"
            | "mechanical_drill"
    ) {
        return Some(27);
    }
    if kinetics::Kind::from_id(path).is_some()
        || matches!(
            path,
            "gearbox"
                | "encased_chain_drive"
                | "fluid_tank"
                | "fluid_pipe"
                | "smart_fluid_pipe"
                | "mechanical_pump"
                | "blaze_burner"
        )
    {
        return Some(0);
    }
    None
}
#[cfg(test)]
mod tests {
use super::*;
fn stack(id: &str, count: u32) -> Option<Stack> {
    Some(Stack {
        id: id.into(),
        count,
        components: String::new(),
    })
}
fn press() -> Vec<Option<Stack>> {
    let mut slots = vec![None; 27];
    slots[0] = stack("minecraft:iron_ingot", 3);
    slots
}
fn finish(state: &MachineState, slots: &[Option<Stack>], limit: impl Fn(&str) -> u32) -> Update {
    let mut next = state.clone();
    for _ in 0..200 {
        let u = process(&next, slots, "pressing", 256., Heat::None, &limit).unwrap();
        next = u.state.clone();
        if matches!(u.result, TickResult::Completed | TickResult::OutputBlocked) {
            return u;
        }
    }
    panic!("recipe never completed")
}
#[test]
fn processes_real_slots_only() {
    let u = finish(&MachineState::default(), &press(), |_| 64);
    assert_eq!(u.result, TickResult::Completed);
    assert_eq!(u.inventory.after[0].as_ref().unwrap().count, 2);
    assert_eq!(u.inventory.after[9], stack("create:iron_sheet", 1));
    assert!(
        serde_json::from_str::<serde_json::Value>(&u.state.encode())
            .unwrap()
            .get("items")
            .is_none()
    );
}
#[test]
fn stale_inventory_cannot_commit() {
    let mut slots = press();
    let u = finish(&MachineState::default(), &slots, |_| 64);
    slots[0] = stack("minecraft:gold_ingot", 1);
    let before = slots.clone();
    assert!(!u.inventory.commit(&mut slots));
    assert_eq!(slots, before);
}
#[test]
fn component_inputs_are_preserved() {
    let mut slots = press();
    slots[0].as_mut().unwrap().components = "{custom:1}".into();
    let u = process(
        &MachineState::default(),
        &slots,
        "pressing",
        256.,
        Heat::None,
        |_| 64,
    )
    .unwrap();
    assert_eq!(u.result, TickResult::Idle);
    assert_eq!(u.inventory.after, slots);
}
#[test]
fn full_output_preserves_inputs_and_rng() {
    let mut slots = press();
    for s in &mut slots[OUTPUT] {
        *s = stack("create:iron_sheet", 1);
    }
    let state = MachineState::default();
    let u = finish(&state, &slots, |_| 1);
    assert_eq!(u.result, TickResult::OutputBlocked);
    assert_eq!(u.inventory.after, slots);
    assert_eq!(u.state.rng, state.rng);
}
#[test]
fn fluid_capacity_and_roundtrip() {
    let mut s = MachineState::default();
    assert!(s.insert_fluid("minecraft:water", 8000));
    assert!(!s.insert_fluid("minecraft:lava", 1));
    assert_eq!(MachineState::decode(&s.encode()).unwrap(), s);
    assert!(!s.extract_fluid("minecraft:water", 8001));
    assert!(s.extract_fluid("minecraft:water", 1000));
    assert_eq!(s.fluid_in["minecraft:water"], 7000);
}
#[test]
fn invalid_saved_data_is_rejected() {
    assert!(MachineState::decode("{\"version\":2}").is_err());
    assert!(MachineState::decode("{\"direction\":0}").is_err());
    assert!(MachineState::decode("{\"fluid_in\":{\"minecraft:water\":8001}}").is_err());
}
#[test]
fn move_preserves_components_and_actual_limits() {
    let mut from = vec![stack("minecraft:diamond_sword", 1)];
    from[0].as_mut().unwrap().components = "{damage:7}".into();
    let target = vec![None; 2];
    let (a, b) = transfer(&from, &target, 1, |_| 1).unwrap();
    assert_eq!(a.after, vec![None]);
    assert_eq!(b.after[0], from[0]);
    assert_eq!(from[0].as_ref().unwrap().count, 1);
}
#[test]
fn blocked_transfer_never_consumes() {
    let from = vec![stack("minecraft:stone", 2)];
    let to = vec![stack("minecraft:stone", 64)];
    assert!(transfer(&from, &to, 1, |_| 64).is_none());
}
#[test]
fn fluid_transfer_conserves_total_and_budget() {
    let mut from = MachineState::default();
    assert!(from.insert_fluid("minecraft:water", 3000));
    let to = MachineState::default();
    let (from, to, n) = transfer_fluid(&from, &to, 120).unwrap();
    assert_eq!(n, 120);
    assert_eq!(
        from.fluid_in["minecraft:water"] + to.fluid_in["minecraft:water"],
        3000
    );
}
#[test]
fn rotor_phase_survives_speed_changes_and_reload() {
    let mut s = MachineState::default();
    s.set_speed(32., 10);
    let angle = s.render_angle(27, 0.);
    s.set_speed(-64., 27);
    assert!((s.render_angle(27, 0.) - angle).abs() < 0.0001);
    assert_eq!(
        MachineState::decode(&s.encode())
            .unwrap()
            .render_angle(45, 0.5),
        s.render_angle(45, 0.5)
    );
}

#[test]
fn geared_press_pipeline_survives_state_reload_and_conserves_items() {
    use kinetics::{Axis, Kind, Node, solve};
    let mut motor = Node::new(Kind::CreativeMotor, Axis::Z);
    motor.configured_rpm = Some(64.0);
    let nodes = BTreeMap::from([
        ([0, 0, 0], motor),
        ([0, 0, 1], Node::new(Kind::LargeCogwheel, Axis::Z)),
        ([1, 1, 1], Node::new(Kind::Cogwheel, Axis::Z)),
        ([1, 1, 2], Node::new(Kind::MechanicalPress, Axis::Z)),
    ]);
    let solution = solve(&nodes);
    let rpm = solution.speeds[&[1, 1, 2]];
    assert_eq!(rpm, -128.0);
    let mut state = MachineState::default();
    let mut slots = press();
    for _ in 0..200 {
        let u = process(&state, &slots, "pressing", rpm, Heat::None, |_| 64).unwrap();
        let result = u.result;
        state = MachineState::decode(&u.state.encode()).unwrap();
        assert!(u.inventory.commit(&mut slots));
        if matches!(result, TickResult::Completed) {
            break;
        }
    }
    assert_eq!(slots[0].as_ref().unwrap().count, 2);
    assert_eq!(slots[9], stack("create:iron_sheet", 1));
}
#[test]
fn stopped_drive_cannot_process_or_consume_input() {
    let before = press();
    let state = MachineState::default();
    let mut slots = before.clone();
    let mut state = state;
    for _ in 0..100 {
        let u = process(&state, &slots, "pressing", 0.0, Heat::None, |_| 64).unwrap();
        state = u.state.clone();
        assert!(u.inventory.commit(&mut slots));
    }
    assert_eq!(slots, before);
    assert_eq!(state.progress, 0.0);
}
#[test]
fn configured_motor_speed_is_validated_when_loading() {
    assert!(MachineState::decode("{\"motor_rpm\":257}").is_err());
    assert!(MachineState::decode("{\"motor_rpm\":-1}").is_err());
    assert_eq!(MachineState::decode("{}").unwrap().motor_rpm, 16.0);
}

}

pub mod worldgen {
// Deterministic prototype zinc veins. The host applies these only to fresh
// overworld chunks and only replaces natural stone/deepslate.
pub fn zinc_positions(seed: i64, cx: i32, cz: i32, min_y: i32, max_y: i32) -> Vec<[i32; 3]> {
    if max_y <= min_y {
        return vec![];
    }
    let mut rng = (seed as u64)
        ^ (cx as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cz as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    let mut next = || {
        rng = rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = rng;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        n ^ (n >> 31)
    };
    let mut out = std::collections::BTreeSet::new();
    for _ in 0..8 {
        let mut p = [
            (next() % 16) as i32,
            (-40 + (next() % 96) as i32).clamp(min_y, max_y - 1),
            (next() % 16) as i32,
        ];
        for _ in 0..6 {
            out.insert([cx * 16 + p[0], p[1], cz * 16 + p[2]]);
            let a = (next() % 3) as usize;
            let d = if next() & 1 == 0 { -1 } else { 1 };
            p[a] = (p[a] + d).clamp(
                if a == 1 { min_y } else { 0 },
                if a == 1 { max_y - 1 } else { 15 },
            );
        }
    }
    out.into_iter().collect()
}
pub fn should_generate(fresh: bool, overworld: bool, already_generated: bool) -> bool {
    fresh && overworld && !already_generated
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_repeatable_veins() {
        let a = zinc_positions(42, -3, 5, -64, 320);
        assert_eq!(a, zinc_positions(42, -3, 5, -64, 320));
        assert!(!a.is_empty());
        assert!(
            a.iter()
                .all(|p| p[0] >> 4 == -3 && p[2] >> 4 == 5 && (-64..320).contains(&p[1]))
        );
        assert_ne!(a, zinc_positions(43, -3, 5, -64, 320));
    }
    #[test]
    fn never_retro_generates_existing_chunks() {
        assert!(!should_generate(false, true, false));
        assert!(!should_generate(true, false, false));
        assert!(!should_generate(true, true, true));
        assert!(should_generate(true, true, false));
    }
}

}

}
use super::{Game, Hit, Screen};
use horizon_client::{
    Result,
    hud::{HudBatch, rgba},
    items::ItemStack,
    mesher::Vertex,
};
use horizon_core::position::BlockPos;
use horizon_world::{fluid::FluidState, fluid_flow::FluidFlowWorld, registry::BlockStateId};
use kinetic::{Axis, Face, FlowSample, Kind, Network, Node, Orientation, Placement, Pos};
use std::{
    cell::Cell,
    collections::BTreeMap,
    sync::{Mutex, OnceLock, atomic::Ordering},
};
thread_local! {static HIT_FACE:Cell<Option<([i32;3],Face)>>=const{Cell::new(None)};}
fn face(v: [i32; 3]) -> Face {
    match v {
        [1, 0, 0] => Face::East,
        [-1, 0, 0] => Face::West,
        [0, 1, 0] => Face::Up,
        [0, -1, 0] => Face::Down,
        [0, 0, -1] => Face::North,
        _ => Face::South,
    }
}
fn face_name(f: Face) -> &'static str {
    match f {
        Face::East => "east",
        Face::West => "west",
        Face::Up => "up",
        Face::Down => "down",
        Face::North => "north",
        Face::South => "south",
    }
}
fn axis_name(a: Axis) -> &'static str {
    match a {
        Axis::X => "x",
        Axis::Y => "y",
        Axis::Z => "z",
    }
}
fn horizontal(yaw: f32) -> Face {
    match ((yaw / 90.0).round() as i32).rem_euclid(4) {
        0 => Face::South,
        1 => Face::West,
        2 => Face::North,
        _ => Face::East,
    }
}
fn read_axis(s: &horizon_world::chunk::BlockState) -> Axis {
    match s.properties.get("axis").map(String::as_str).unwrap_or("y") {
        "x" => Axis::X,
        "z" => Axis::Z,
        _ => Axis::Y,
    }
}
pub(super) fn remember_hit(hit: Option<&Hit>) {
    HIT_FACE.with(|f| f.set(hit.map(|h| (h.pos, face(h.face)))));
}
pub(super) fn placed(
    game: &mut Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
) -> Result<BlockStateId> {
    let Some(s) = game.pack.assets.registry.state(state) else {
        return Ok(state);
    };
    let name = s.name.clone();
    if !name.starts_with("create:") {
        return Ok(state);
    }
    let Some((clicked, clicked_face)) = HIT_FACE.with(Cell::get) else {
        return Ok(state);
    };
    let normal = match clicked_face {
        Face::East => [1, 0, 0],
        Face::West => [-1, 0, 0],
        Face::Up => [0, 1, 0],
        Face::Down => [0, -1, 0],
        Face::North => [0, 0, -1],
        Face::South => [0, 0, 1],
    };
    if pos != clicked
        && pos
            != [
                clicked[0] + normal[0],
                clicked[1] + normal[1],
                clicked[2] + normal[2],
            ]
    {
        return Ok(state);
    };

    let sneak = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let horizontal = horizontal(game.yaw);
    let look = if game.pitch > 45.0 {
        Face::Down
    } else if game.pitch < -45.0 {
        Face::Up
    } else {
        horizontal
    };
    let kind = if s.properties.contains_key("facing") {
        if matches!(
            name.as_str(),
            "create:mechanical_press" | "create:mechanical_mixer" | "create:millstone"
        ) {
            Placement::Upright
        } else {
            Placement::Directional
        }
    } else if s.properties.contains_key("axis") {
        if name.ends_with("water_wheel") {
            Placement::HorizontalAxial
        } else {
            Placement::Axial
        }
    } else {
        return Ok(state);
    };
    let mut neighbour = None;
    for d in [
        [0, 1, 0],
        [0, -1, 0],
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let p = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
        if let Some(n) = game
            .pack
            .assets
            .registry
            .state(game.world.block(p[0], p[1], p[2]))
        {
            if n.name.starts_with("create:")
                && n.properties.contains_key("axis")
                && read_axis(n) == face(d).axis()
            {
                neighbour = Some(read_axis(n));
                break;
            }
        }
    }
    let orientation = kinetic::placement(kind, clicked_face, look, horizontal, sneak, neighbour);
    let orientation = if name == "create:gearbox" {
        let axis = clicked_face.axis();
        let axis = if neighbour == Some(axis) {
            if axis == Axis::Y { Axis::X } else { Axis::Y }
        } else {
            axis
        };
        Orientation::Axis(axis)
    } else {
        orientation
    };
    let (key, value) = match orientation {
        Orientation::Axis(a) => ("axis", axis_name(a)),
        Orientation::Facing(f) => ("facing", face_name(f)),
    };
    let new = game.with_props(state, &[(key, value)]).unwrap_or(state);
    if new != state {
        game.level_set(pos, new)?;
    }
    Ok(new)
}
pub(super) fn wrench_block(game: &mut Game<'_>, pos: [i32; 3]) -> Result<bool> {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let Some(s) = game.pack.assets.registry.state(state) else {
        return Ok(false);
    };
    if !s.name.starts_with("create:") {
        return Ok(false);
    }
    let (key, values): (&str, &[&str]) = if s.properties.contains_key("facing") {
        if matches!(
            s.name.as_str(),
            "create:mechanical_press" | "create:mechanical_mixer" | "create:millstone"
        ) {
            ("facing", &["south", "west", "north", "east"])
        } else {
            ("facing", &["south", "west", "north", "east", "up", "down"])
        }
    } else if s.properties.contains_key("axis") {
        ("axis", &["x", "y", "z"])
    } else {
        return Ok(false);
    };
    let index = values
        .iter()
        .position(|v| s.properties.get(key).is_some_and(|s| s == v))
        .unwrap_or(0);
    let step = if game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) {
        values.len() - 1
    } else {
        1
    };
    let next = values[(index + step) % values.len()];
    if let Some(new) = game.with_props(state, &[(key, next)]) {
        game.level_set(pos, new)?;
        return Ok(true);
    }
    Ok(false)
}
struct Water<'a, 'b> {
    game: &'a Game<'b>,
}
impl FluidFlowWorld for Water<'_, '_> {
    type Error = std::io::Error;
    type Block = BlockStateId;
    fn fluid(&mut self, p: BlockPos) -> std::io::Result<FluidState> {
        Ok(self
            .game
            .res
            .behaviours()
            .fluid(self.game.world.block(p.x, p.y, p.z)))
    }
    fn block(&mut self, p: BlockPos) -> std::io::Result<BlockStateId> {
        Ok(self.game.world.block(p.x, p.y, p.z))
    }
    fn blocks_fluid_flow(&self, s: BlockStateId) -> bool {
        self.game
            .pack
            .assets
            .registry
            .state(s)
            .is_some_and(|s| self.game.pack.assets.blocks_fluid_flow.contains(&s.name))
    }
    fn is_ice_class(&self, s: BlockStateId) -> bool {
        self.game
            .res
            .facts()
            .get(s)
            .classes
            .iter()
            .any(|c| c.as_ref() == "IceBlock")
    }
    fn is_face_sturdy(
        &mut self,
        s: BlockStateId,
        _: BlockPos,
        d: horizon_core::geometry::Direction,
    ) -> std::io::Result<bool> {
        Ok(self.game.res.facts().get(s).sturdy_full & (1 << d as u8) != 0)
    }
}
#[derive(Default)]
struct Cache {
    world: usize,
    sections: BTreeMap<
        (i32, usize, i32),
        (
            std::sync::Arc<horizon_client::chunk::Section>,
            Vec<(Pos, u32)>,
        ),
    >,
    network: Network,
    last_time: f64,
}
fn dynamic(name: &str) -> bool {
    matches!(
        name,
        "create:shaft"
            | "create:water_wheel"
            | "create:large_water_wheel"
            | "create:cogwheel"
            | "create:large_cogwheel"
            | "create:crushing_wheel"
    )
}
fn refresh(game: &Game<'_>, cache: &mut Cache) {
    let world = &game.world as *const _ as usize;
    if cache.world != world {
        *cache = Cache {
            world,
            ..Cache::default()
        };
    }
    let mut live = std::collections::BTreeSet::new();
    let mut nodes = BTreeMap::new();
    for (&(cx, cz), chunk) in &game.world.chunks {
        for (sy, section) in chunk.sections.iter().enumerate() {
            let key = (cx, sy, cz);
            live.insert(key);
            let token = std::sync::Arc::clone(section);
            let changed = cache
                .sections
                .get(&key)
                .is_none_or(|(old, _)| !std::sync::Arc::ptr_eq(old, &token));
            if changed {
                let mut found = vec![];
                // Skip uniform air/stone sections before walking the palette.
                if !section.blocks.uniform().is_some_and(|id| {
                    game.pack
                        .assets
                        .registry
                        .state(BlockStateId(id))
                        .is_none_or(|s| !dynamic(&s.name) && s.name != "create:creative_motor")
                }) {
                    for i in 0..4096 {
                        let id = section.get(i);
                        let Some(s) = game.pack.assets.registry.state(BlockStateId(id)) else {
                            continue;
                        };
                        if dynamic(&s.name) || s.name == "create:creative_motor" {
                            let pos = Pos(
                                cx * 16 + (i & 15) as i32,
                                chunk.min_y() + sy as i32 * 16 + (i >> 8) as i32,
                                cz * 16 + ((i >> 4) & 15) as i32,
                            );
                            found.push((pos, id));
                        }
                    }
                }
                cache.sections.insert(key, (token, found));
            }
            for &(pos, id) in &cache.sections[&key].1 {
                let s = game.pack.assets.registry.state(BlockStateId(id)).unwrap();
                let axis = if s.name == "create:creative_motor" {
                    match s.properties.get("facing").map(String::as_str) {
                        Some("east" | "west") => Axis::X,
                        Some("north" | "south") => Axis::Z,
                        _ => Axis::Y,
                    }
                } else {
                    read_axis(s)
                };
                let kind = if s.name == "create:shaft" {
                    Kind::Shaft
                } else if s.name == "create:creative_motor" {
                    Kind::CreativeMotor(16.0)
                } else {
                    Kind::WaterWheel
                };
                let mut node = Node::new(axis, kind);
                if let Some(old) = cache.network.nodes.get(&pos) {
                    node.angle = old.angle;
                }
                nodes.insert(pos, node);
            }
        }
    }
    cache.sections.retain(|k, _| live.contains(k));
    cache.network.nodes = nodes;
}
fn flows(game: &Game<'_>, network: &Network) -> BTreeMap<Pos, Vec<FlowSample>> {
    let mut out = BTreeMap::new();
    let mut water = Water { game };
    for (&pos, node) in &network.nodes {
        if !matches!(node.kind, Kind::WaterWheel) {
            continue;
        }
        let s = game
            .pack
            .assets
            .registry
            .state(game.world.block(pos.0, pos.1, pos.2))
            .unwrap();
        let radius: i32 = if s.name == "create:large_water_wheel" {
            2
        } else {
            1
        };
        let mut samples = vec![];
        for u in -radius..=radius {
            for v in -radius..=radius {
                if u.abs() != radius && v.abs() != radius {
                    continue;
                }
                let offset = match node.axis {
                    Axis::X => [0, u, v],
                    Axis::Y => [u, 0, v],
                    Axis::Z => [u, v, 0],
                };
                let p = BlockPos::new(pos.0 + offset[0], pos.1 + offset[1], pos.2 + offset[2]);
                let Ok(fluid) = water.fluid(p) else { continue };
                if fluid.kind() != horizon_world::fluid::FluidKind::Water {
                    continue;
                }
                if let Ok(flow) = horizon_world::fluid_flow::flow_at(&mut water, fluid, p) {
                    samples.push(FlowSample {
                        offset: offset.map(f64::from),
                        velocity: [flow.x, flow.y, flow.z],
                    });
                }
            }
        }
        out.insert(pos, samples);
    }
    out
}
pub(super) fn draw_rotation(
    game: &mut Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
) {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let Ok(mut cache) = CACHE.get_or_init(|| Mutex::new(Cache::default())).lock() else {
        return;
    };
    refresh(game, &mut cache);
    let time = game.world.game_time as f64 + alpha;
    let dt = if time >= cache.last_time {
        (time - cache.last_time).min(20.0) / 20.0
    } else {
        0.0
    };
    cache.last_time = time;
    let _ = dt;
    for (&pos, node) in &cache.network.nodes {
        let at =
            glam::DVec3::new(pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5) - camera;
        if at.length_squared() > 96.0 * 96.0 {
            continue;
        }
        let state_id = game.world.block(pos.0, pos.1, pos.2);
        let Some(state) = game.pack.assets.registry.state(state_id) else {
            continue;
        };
        if !dynamic(&state.name) {
            continue;
        }
        let item = ItemStack::new(&state.name, 1);
        let model = game.resolve_item(
            &item,
            horizon_assets::item_model::DisplayContext::None,
            None,
            0,
        );
        let rotation = glam::Mat4::from_axis_angle(
            match node.axis {
                Axis::X => glam::Vec3::X,
                Axis::Y => glam::Vec3::Y,
                Axis::Z => glam::Vec3::Z,
            },
            read_machine(game, (pos.0, pos.1, pos.2))
                .render_angle(game.world.game_time.max(0) as u64, alpha as f32)
                .to_radians(),
        );
        let orientation = match node.axis {
            Axis::Y => glam::Mat4::IDENTITY,
            Axis::X => glam::Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            Axis::Z => glam::Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
        };
        let pose = glam::Mat4::from_translation(at.as_vec3()) * rotation * orientation;
        game.submit_item_world(
            &model,
            pose,
            horizon_client::block_entity_render::vertex_light(
                game.world.light(pos.0, pos.1, pos.2),
            ),
            out,
        );
    }
}
pub(super) fn draw_pages(game: &mut Game<'_>, batch: &mut HudBatch) {
    if game.screen != Screen::Creative
        || !game
            .creative_screen
            .tabs
            .iter()
            .any(|t| t.id == "create:main")
    {
        return;
    }
    let (x, y) = Game::creative_origin(batch);
    let page = horizon_client::mod_content::create_preview_support::PAGE.load(Ordering::Relaxed);
    batch.fill(
        &game.gui.atlas,
        x + 65.0,
        y - 48.0,
        65.0,
        17.0,
        rgba(35, 35, 35, 255),
    );
    batch.text(
        &game.gui.atlas,
        &format!("<   {} / 2   >", page + 1),
        x + 68.0,
        y - 44.0,
        u32::MAX,
        false,
    );
}
pub(super) fn click_page(game: &mut Game<'_>) -> bool {
    let Some(r) = &game.renderer else {
        return false;
    };
    let batch = HudBatch::new([r.config.width, r.config.height]);
    let (x, y) = Game::creative_origin(&batch);
    let (mx, my) = game.cursor;
    if mx < x + 65.0 || mx >= x + 130.0 || my < y - 48.0 || my >= y - 31.0 {
        return false;
    }
    let page = &horizon_client::mod_content::create_preview_support::PAGE;
    let target = 1 - page.load(Ordering::Relaxed);
    let index = game.creative_screen.tabs.iter().position(|t| {
        if target == 1 {
            t.id == "create:main"
        } else {
            t.id != "create:main" && !t.search && !t.inventory
        }
    });
    let Some(index) = index else { return false };
    page.store(target, Ordering::Relaxed);
    super::select_creative_tab(&mut game.creative_screen, &mut game.inv_clicks, index);
    game.inv_drag = None;
    game.last_inv_click = None;
    true
}

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
fn process_at(game: &mut Game<'_>, p: MachinePos, station: &str, rpm: f32, heat: machines::Heat) {
    process_with_fluid_owner(game, p, p, station, rpm, heat)
}
fn process_with_fluid_owner(
    game: &mut Game<'_>,
    p: MachinePos,
    owner: MachinePos,
    station: &str,
    rpm: f32,
    heat: machines::Heat,
) {
    ensure_machine(game, p);
    let before = match game.world.block_entities.get(&p) {
        Some(e) => e.slots().to_vec(),
        None => return,
    };
    if before.len() < 18 {
        return;
    }
    let mut state = read_machine(game, owner);
    state.set_speed(rpm, game.world.game_time.max(0) as u64);
    let input = portable(&before);
    let catalog: Vec<_> = recipes::catalog::recipes()
        .iter()
        .filter(|r| {
            r.station == station
                && r.products
                    .iter()
                    .all(|p| p.fluid || game.res.items().get(p.id).is_some())
        })
        .copied()
        .collect();
    let Ok(update) =
        machines::process_with_catalog(&state, &input, station, rpm, heat, &catalog, |id| {
            game.res.items().get(id).map_or(1, |i| i.max_stack)
        })
    else {
        return;
    };
    if update
        .inventory
        .after
        .iter()
        .flatten()
        .any(|s| game.res.items().get(&s.id).is_none())
    {
        return;
    }
    let Some(after) = restore(&update.inventory.after, &before) else {
        return;
    };
    if let Some(entity) = game.world.block_entities.get_mut(&p) {
        if portable(entity.slots()) != update.inventory.before {
            return;
        }
        let changed = entity.slots() != after;
        entity.slots_mut().clone_from_slice(&after);
        if changed {
            dirty_machine(game, p);
        }
    }
    store_machine(game, owner, update.state);
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
            if let Ok(data) = serde_json::to_vec(&(1u32, format!("{:?}", game.dim), batch, notice))
            {
                if data.len() < 1_000_000 {
                    lan.hub.broadcast_level(
                        &horizon_protocol::packets::play::Clientbound::CustomPayload(
                            horizon_protocol::packets::configuration::CustomPayload {
                                channel: "create:machines_v1".into(),
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
    if payload.channel != "create:machines_v1" {
        return false;
    }
    if payload.data.len() > 1_000_000 {
        return true;
    }
    if let Ok((version, dim, values, notice)) =
        serde_json::from_slice::<(u32, String, Vec<MachineRecord>, String)>(&payload.data)
    {
        if version != 1 || dim != format!("{:?}", game.dim) || values.len() > 128 {
            return true;
        }
        for r in values {
            if !r.name.starts_with("create:")
                || machines::MachineState::decode(&r.data.encode()).is_err()
            {
                continue;
            }
            let p = (r.pos[0], r.pos[1], r.pos[2]);
            if machine_name(game, p) == r.name {
                store_machine(game, p, r.data);
            }
        }
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
    thread_local! {static LAST:std::cell::RefCell<BTreeMap<std::path::PathBuf,i64>>=const{std::cell::RefCell::new(BTreeMap::new())};}
    let tick = game.world.game_time.max(0) as u64;
    let duplicate = LAST
        .with(|m| m.borrow_mut().insert(game.save.root.clone(), tick as i64) == Some(tick as i64));
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
        node.direction = saved.direction;
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
                    _ => continue,
                };
            }
            if matches!(
                name.as_str(),
                "create:mechanical_press" | "create:mechanical_mixer"
            ) {
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
            if name == "create:spout" && machine_name(game, below) == "create:depot" {
                process_with_fluid_owner(game, below, *p, station, rpm, heat);
            } else {
                process_at(game, target, station, rpm, heat);
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
    if tick.is_multiple_of(5) {
        let values: Vec<_> = positions
            .iter()
            .map(|(p, name)| MachineRecord {
                pos: [p.0, p.1, p.2],
                name: name.clone(),
                data: read_machine(game, *p),
            })
            .collect();
        broadcast_machine_state(game, &values, "");
    }
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
    let Some(pos) = game.container else { return };
    let name = machine_name(game, pos);
    if !name.starts_with("create:") {
        return;
    }
    let s = read_machine(game, pos);
    let (x, y) = game.inventory_origin(batch);
    batch.fill(
        &game.gui.atlas,
        x,
        y - 30.0,
        176.0,
        27.0,
        rgba(25, 25, 25, 230),
    );
    batch.text(
        &game.gui.atlas,
        &format!("{:.0} RPM | {}", s.rpm, s.status),
        x + 4.0,
        y - 26.0,
        u32::MAX,
        false,
    );
    batch.text(
        &game.gui.atlas,
        &format!(
            "Work {:.0} | Fluid {} mB",
            s.progress,
            s.fluid_in.values().chain(s.fluid_out.values()).sum::<u32>()
        ),
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
