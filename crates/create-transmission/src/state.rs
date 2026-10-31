//! Reading Horizon's block state strings (`create:shaft[axis=y]`) into
//! kinetic nodes.

use crate::{Axis, Kind, Node};

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
            Kind::Belt => {
                if axis == Axis::Y {
                    return None;
                }
                node.axis = if axis == Axis::X { Axis::Z } else { Axis::X };
                node.ports = 3 << (node.axis.index() * 2);
                node.direction = if positive { 1 } else { -1 };
            }
            Kind::MechanicalCrafter => {
                if axis == Axis::Y {
                    return None;
                }
                node.axis = axis;
                node.ports = 3 << (axis.index() * 2);
            }
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
