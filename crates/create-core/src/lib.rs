//! Engine-independent prototype. No Horizon runtime adapter exists yet.
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
