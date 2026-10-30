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
