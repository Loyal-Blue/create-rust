//! Flat belt and funnel rules. Item entities/inventories remain owned by the host.
use super::{INPUT, OUTPUT, Stack};
pub type Pos = [i32; 3];
pub const BELT_SURFACE: f64 = 13.0 / 16.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    East,
    West,
}
impl Facing {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "north" => Self::North,
            "south" => Self::South,
            "east" => Self::East,
            "west" => Self::West,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
        }
    }
    pub fn step(self) -> Pos {
        match self {
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::East => [1, 0, 0],
            Self::West => [-1, 0, 0],
        }
    }
    pub fn shaft_axis(self) -> super::kinetics::Axis {
        if self.step()[0] != 0 {
            super::kinetics::Axis::Z
        } else {
            super::kinetics::Axis::X
        }
    }
    pub fn sign(self) -> i8 {
        if matches!(self, Self::North | Self::West) {
            -1
        } else {
            1
        }
    }
}
/// A validated, bounded straight connection; the host checks replaceability
/// and loaded chunks for the complete plan before it places anything.
pub fn connection(start: Pos, end: Pos, axis: super::kinetics::Axis) -> Option<(Facing, Vec<Pos>)> {
    let d = [
        end[0].checked_sub(start[0])?,
        end[1].checked_sub(start[1])?,
        end[2].checked_sub(start[2])?,
    ];
    if d[1] != 0 || (d[0] != 0) == (d[2] != 0) {
        return None;
    }
    let facing = if d[0] > 0 {
        Facing::East
    } else if d[0] < 0 {
        Facing::West
    } else if d[2] > 0 {
        Facing::South
    } else {
        Facing::North
    };
    if facing.shaft_axis() != axis {
        return None;
    }
    let distance = d[0].unsigned_abs().max(d[2].unsigned_abs());
    if !(1..=16).contains(&distance) {
        return None;
    }
    let step = facing.step();
    let mut positions = Vec::new();
    for n in 0..=distance as i32 {
        positions.push([
            start[0].checked_add(step[0] * n)?,
            start[1],
            start[2].checked_add(step[2] * n)?,
        ]);
    }
    Some((facing, positions))
}
/// Set horizontal speed each tick, including zero when stopped. A weak
/// centering velocity prevents successive seams from pushing stacks sideways.
pub fn belt_velocity(facing: Facing, rpm: f32, local: [f64; 3]) -> Option<[f64; 2]> {
    if !rpm.is_finite()
        || !local.iter().all(|v| v.is_finite())
        || !(BELT_SURFACE - 0.04..=BELT_SURFACE + 0.30).contains(&local[1])
        || !(0.0..1.0).contains(&local[0])
        || !(0.0..1.0).contains(&local[2])
    {
        return None;
    }
    let step = facing.step();
    let speed = f64::from(rpm.clamp(-256.0, 256.0)) / 512.0;
    let center = if rpm == 0.0 {
        0.0
    } else if step[0] != 0 {
        (0.5 - local[2]) * 0.15
    } else {
        (0.5 - local[0]) * 0.15
    };
    Some(if step[0] != 0 {
        [speed * f64::from(step[0]), center]
    } else {
        [center, speed * f64::from(step[2])]
    })
}
/// A mouth only reaches into its own lane, never through the attached inventory.
pub fn at_mouth(facing: Facing, relative: [f64; 3]) -> bool {
    if !relative.iter().all(|v| v.is_finite()) {
        return false;
    }
    let step = facing.step();
    let along = relative[0] * f64::from(step[0]) + relative[2] * f64::from(step[2]);
    let across = if step[0] != 0 {
        relative[2]
    } else {
        relative[0]
    };
    (0.0..=1.6).contains(&along) && across.abs() <= 0.45 && relative[1].abs() <= 0.5
}
pub fn filter_accepts(filter: Option<&str>, stack: &Stack) -> bool {
    filter.is_none_or(|id| id == stack.id)
}
pub fn source_slots(name: &str, len: usize) -> std::ops::Range<usize> {
    if super::station(name).is_some() || matches!(name, "create:basin" | "create:depot") {
        OUTPUT.start.min(len)..OUTPUT.end.min(len)
    } else {
        0..len
    }
}
pub fn input_slots(name: &str, len: usize) -> std::ops::Range<usize> {
    if super::station(name).is_some() || matches!(name, "create:basin" | "create:depot") {
        0..INPUT.end.min(len)
    } else {
        0..len
    }
}
pub fn supported_container(name: &str) -> bool {
    (name.starts_with("create:") && super::container_size(name).is_some_and(|n| n >= 18))
        || matches!(
            name,
            "minecraft:chest"
                | "minecraft:trapped_chest"
                | "minecraft:barrel"
                | "minecraft:hopper"
                | "minecraft:dispenser"
                | "minecraft:dropper"
        )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connector_checks_axis_length_and_corners() {
        let (f, p) = connection([0, 5, 0], [0, 5, 4], super::super::kinetics::Axis::X).unwrap();
        assert_eq!(f, Facing::South);
        assert_eq!(p.len(), 5);
        assert!(connection([0, 0, 0], [2, 0, 2], super::super::kinetics::Axis::X).is_none());
        assert!(connection([0, 0, 0], [0, 0, 17], super::super::kinetics::Axis::X).is_none());
        assert!(connection([0, 0, 0], [0, 1, 4], super::super::kinetics::Axis::X).is_none());
        assert!(connection([0, 0, 0], [0, 0, 4], super::super::kinetics::Axis::Z).is_none());
    }
    #[test]
    fn belt_reverses_stops_and_ignores_airborne_items() {
        let local = [0.5, BELT_SURFACE, 0.5];
        assert_eq!(
            belt_velocity(Facing::South, 64.0, local),
            Some([0.0, 0.125])
        );
        assert_eq!(
            belt_velocity(Facing::South, -64.0, local),
            Some([0.0, -0.125])
        );
        assert_eq!(belt_velocity(Facing::South, 0.0, local), Some([0.0, 0.0]));
        assert!(belt_velocity(Facing::South, 64.0, [0.5, 1.5, 0.5]).is_none());
        assert!(belt_velocity(Facing::South, f32::NAN, local).is_none());
    }
    #[test]
    fn machine_exports_do_not_steal_unprocessed_inputs() {
        assert_eq!(source_slots("create:mechanical_press", 27), 9..18);
        assert_eq!(source_slots("create:basin", 27), 9..18);
        assert_eq!(input_slots("create:millstone", 27), 0..9);
        assert_eq!(source_slots("minecraft:chest", 27), 0..27);
        assert!(!supported_container("minecraft:furnace"));
        assert!(!supported_container("minecraft:shulker_box"));
    }
    #[test]
    fn filtering_keeps_component_metadata() {
        let stack = Stack {
            id: "minecraft:iron_ingot".into(),
            count: 9,
            components: "{name:1}".into(),
        };
        assert!(filter_accepts(Some("minecraft:iron_ingot"), &stack));
        assert!(!filter_accepts(Some("minecraft:gold_ingot"), &stack));
        assert!(filter_accepts(None, &stack));
        assert_eq!(stack.components, "{name:1}");
    }
}

#[cfg(test)]
mod mouth_tests {
    use super::*;
    #[test]
    fn mouths_ignore_neighbor_lanes_and_items_behind() {
        assert!(at_mouth(Facing::South, [0.0, 0.0, 0.8]));
        assert!(at_mouth(Facing::North, [0.0, 0.0, -0.8]));
        assert!(!at_mouth(Facing::South, [1.0, 0.0, 0.8]));
        assert!(!at_mouth(Facing::South, [0.0, 0.0, -0.8]));
    }
}
