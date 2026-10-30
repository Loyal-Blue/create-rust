//! Bounded, conservative routing through a loaded pipe graph. No world cells
//! are loaded by the planner and no pipe contains a second copy of fluid.
use super::{MachineState, TANK_CAPACITY};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub type Pos = [i32; 3];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    Pipe,
    Port { single: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    NoPower,
    NoInlet,
    NoOutlet,
    ConnectedLoop,
    TooLarge,
    Blocked,
    Flowing,
}
#[derive(Debug)]
pub struct Flow {
    pub states: BTreeMap<Pos, MachineState>,
    pub amount: u32,
    pub status: Status,
}
const STEPS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
fn add(p: Pos, d: Pos) -> Option<Pos> {
    Some([
        p[0].checked_add(d[0])?,
        p[1].checked_add(d[1])?,
        p[2].checked_add(d[2])?,
    ])
}
fn scan(nodes: &BTreeMap<Pos, Node>, start: Pos) -> Result<Vec<Pos>, Status> {
    let mut queue = VecDeque::from([(start, 1u32)]);
    let mut seen = BTreeSet::new();
    let mut ports = BTreeSet::new();
    while let Some((p, distance)) = queue.pop_front() {
        if distance > 16 || !seen.insert(p) {
            continue;
        }
        if seen.len() > 256 {
            return Err(Status::TooLarge);
        }
        match nodes.get(&p) {
            Some(Node::Port { .. }) => {
                ports.insert(p);
            }
            Some(Node::Pipe) => {
                for step in STEPS {
                    if let Some(q) = add(p, step)
                        && nodes.contains_key(&q)
                    {
                        queue.push_back((q, distance + 1));
                    }
                }
            }
            None => {}
        }
    }
    Ok(ports.into_iter().collect())
}
pub fn endpoints(
    nodes: &BTreeMap<Pos, Node>,
    pump: Pos,
    axis: super::kinetics::Axis,
    rpm: f32,
) -> Result<(Vec<Pos>, Vec<Pos>), Status> {
    if !rpm.is_finite() || rpm == 0.0 {
        return Err(Status::NoPower);
    }
    let mut step = [0; 3];
    step[axis.index()] = if rpm > 0.0 { 1 } else { -1 };
    let input = add(pump, step.map(|v| -v)).ok_or(Status::NoInlet)?;
    let output = add(pump, step).ok_or(Status::NoOutlet)?;
    let a = scan(nodes, input)?;
    let b = scan(nodes, output)?;
    if a.is_empty() {
        return Err(Status::NoInlet);
    }
    if b.is_empty() {
        return Err(Status::NoOutlet);
    }
    if a.iter().any(|p| b.contains(p)) {
        return Err(Status::ConnectedLoop);
    }
    Ok((a, b))
}
pub fn can_accept(state: &MachineState, fluid: &str, amount: u32, single: bool) -> bool {
    if amount == 0 {
        return false;
    }
    let input: u64 = state.fluid_in.values().map(|v| u64::from(*v)).sum();
    if !single {
        return input + u64::from(amount) <= u64::from(TANK_CAPACITY);
    }
    let output: u64 = state.fluid_out.values().map(|v| u64::from(*v)).sum();
    input + output + u64::from(amount) <= u64::from(TANK_CAPACITY)
        && state
            .fluid_in
            .iter()
            .chain(&state.fluid_out)
            .all(|(id, n)| *n == 0 || id == fluid)
}
pub fn move_fluid(
    source: &MachineState,
    dest: &MachineState,
    budget: u32,
    single: bool,
) -> Option<(MachineState, MachineState, u32)> {
    let (id, _) = source
        .fluid_out
        .iter()
        .chain(&source.fluid_in)
        .find(|(_, n)| **n > 0)?;
    let total = u64::from(source.fluid_in.get(id).copied().unwrap_or(0))
        + u64::from(source.fluid_out.get(id).copied().unwrap_or(0));
    let occupied: u64 = dest
        .fluid_in
        .values()
        .chain(
            if single {
                Some(dest.fluid_out.values())
            } else {
                None
            }
            .into_iter()
            .flatten(),
        )
        .map(|v| u64::from(*v))
        .sum();
    let room = u64::from(TANK_CAPACITY).saturating_sub(occupied);
    let amount = total.min(u64::from(budget)).min(room) as u32;
    if !can_accept(dest, id, amount, single) {
        return None;
    }
    let mut from = source.clone();
    let mut to = dest.clone();
    if !from.extract_fluid(id, amount) || !to.insert_fluid(id, amount) {
        return None;
    }
    Some((from, to, amount))
}
pub fn flow(
    nodes: &BTreeMap<Pos, Node>,
    states: &BTreeMap<Pos, MachineState>,
    pump: Pos,
    axis: super::kinetics::Axis,
    rpm: f32,
    tick: u64,
) -> Flow {
    let (mut input, mut output) = match endpoints(nodes, pump, axis, rpm) {
        Ok(v) => v,
        Err(status) => {
            return Flow {
                states: BTreeMap::new(),
                amount: 0,
                status,
            };
        }
    };
    let n = input.len();
    input.rotate_left((tick % n as u64) as usize);
    let n = output.len();
    output.rotate_left((tick % n as u64) as usize);
    let mut live = states.clone();
    let mut changed = BTreeSet::new();
    let budget = (rpm.abs().min(256.0) * 4.0).round().max(1.0) as u32;
    let mut left = budget;
    for from in input {
        for (i, to) in output.iter().enumerate() {
            if left == 0 {
                break;
            }
            let share = left.div_ceil((output.len() - i) as u32);
            let (Some(source), Some(dest)) = (live.get(&from), live.get(to)) else {
                continue;
            };
            let single = matches!(nodes.get(to), Some(Node::Port { single: true }));
            if let Some((a, b, n)) = move_fluid(source, dest, share, single) {
                live.insert(from, a);
                live.insert(*to, b);
                changed.insert(from);
                changed.insert(*to);
                left -= n;
            }
        }
    }
    let amount = budget - left;
    Flow {
        states: live
            .into_iter()
            .filter(|(p, _)| changed.contains(p))
            .collect(),
        amount,
        status: if amount == 0 {
            Status::Blocked
        } else {
            Status::Flowing
        },
    }
}
#[cfg(test)]
mod tests {
    use super::super::kinetics::Axis;
    use super::*;
    fn water(n: u32) -> MachineState {
        let mut s = MachineState::default();
        assert!(s.insert_fluid("minecraft:water", n));
        s
    }
    fn network() -> BTreeMap<Pos, Node> {
        BTreeMap::from([
            ([-1, 0, 0], Node::Port { single: true }),
            ([1, 0, 0], Node::Pipe),
            ([2, 0, 0], Node::Pipe),
            ([2, 0, 1], Node::Port { single: true }),
            ([2, 0, -1], Node::Port { single: true }),
        ])
    }
    #[test]
    fn branches_share_one_conserved_pump_budget() {
        let nodes = network();
        let states = BTreeMap::from([
            ([-1, 0, 0], water(1000)),
            ([2, 0, 1], MachineState::default()),
            ([2, 0, -1], MachineState::default()),
        ]);
        let result = flow(&nodes, &states, [0, 0, 0], Axis::X, 32.0, 0);
        assert_eq!(result.amount, 128);
        assert_eq!(result.states[&[-1, 0, 0]].fluid_in["minecraft:water"], 872);
        assert_eq!(result.states[&[2, 0, 1]].fluid_in["minecraft:water"], 64);
        assert_eq!(result.states[&[2, 0, -1]].fluid_in["minecraft:water"], 64);
    }
    #[test]
    fn reversing_uses_the_other_inlet() {
        let nodes = network();
        let states = BTreeMap::from([
            ([-1, 0, 0], MachineState::default()),
            ([2, 0, 1], water(1000)),
            ([2, 0, -1], MachineState::default()),
        ]);
        let result = flow(&nodes, &states, [0, 0, 0], Axis::X, -32.0, 0);
        assert_eq!(result.amount, 128);
        assert_eq!(result.states[&[-1, 0, 0]].fluid_in["minecraft:water"], 128);
    }
    #[test]
    fn a_pipe_loop_never_copies_fluid() {
        let mut nodes = network();
        for p in [[-1, 0, 1], [0, 0, 1], [1, 0, 1]] {
            nodes.insert(p, Node::Pipe);
        }
        assert_eq!(
            endpoints(&nodes, [0, 0, 0], Axis::X, 32.0).unwrap_err(),
            Status::ConnectedLoop
        );
    }
    #[test]
    fn tanks_reject_mixing_and_count_both_saved_stores() {
        let mut source = water(2000);
        let mut dest = water(7000);
        dest.fluid_out.insert("minecraft:water".into(), 500);
        let (_, to, n) = move_fluid(&source, &dest, 2000, true).unwrap();
        assert_eq!(n, 500);
        assert_eq!(to.fluid_in["minecraft:water"], 7500);
        source.fluid_in.clear();
        source.fluid_in.insert("minecraft:lava".into(), 1000);
        assert!(move_fluid(&source, &dest, 1000, true).is_none());
        assert!(move_fluid(&source, &dest, 1000, false).is_some());
    }
    #[test]
    fn extracting_a_bucket_can_span_input_and_output() {
        let mut s = water(600);
        s.fluid_out.insert("minecraft:water".into(), 400);
        assert!(s.extract_fluid("minecraft:water", 1000));
        assert!(s.fluid_in.is_empty());
        assert!(s.fluid_out.is_empty());
        let old = s.clone();
        assert!(!s.extract_fluid("minecraft:water", 1));
        assert_eq!(s, old);
    }
    #[test]
    fn unloaded_gaps_and_disconnected_ports_do_not_route() {
        let mut nodes = network();
        nodes.remove(&[2, 0, 0]);
        assert_eq!(
            endpoints(&nodes, [0, 0, 0], Axis::X, 32.0).unwrap_err(),
            Status::NoOutlet
        );
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::super::kinetics::Axis;
    use super::*;
    #[test]
    fn oversized_pipe_components_stop_without_mutating_storage() {
        let mut nodes = BTreeMap::from([([-1, 0, 0], Node::Port { single: true })]);
        for x in 1..=12 {
            for y in -2..=2 {
                for z in -8..=8 {
                    nodes.insert([x, y, z], Node::Pipe);
                }
            }
        }
        assert_eq!(
            endpoints(&nodes, [0, 0, 0], Axis::X, 32.0).unwrap_err(),
            Status::TooLarge
        );
    }
    #[test]
    fn endpoints_are_limited_to_sixteen_steps() {
        let mut nodes = BTreeMap::from([
            ([-1, 0, 0], Node::Port { single: true }),
            ([17, 0, 0], Node::Port { single: true }),
        ]);
        for x in 1..=16 {
            nodes.insert([x, 0, 0], Node::Pipe);
        }
        assert_eq!(
            endpoints(&nodes, [0, 0, 0], Axis::X, 32.0).unwrap_err(),
            Status::NoOutlet
        );
        nodes.insert([16, 0, 0], Node::Port { single: true });
        assert!(endpoints(&nodes, [0, 0, 0], Axis::X, 32.0).is_ok());
    }
}
