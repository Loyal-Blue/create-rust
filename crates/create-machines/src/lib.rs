//! Native machine transactions over the host's real inventory slots.
//! Saved state contains progress and fluids, never a second copy of items.
pub use create_kinetics as kinetics;
pub use create_simulation::{Heat, TickResult};
use create_simulation::{Machine, Stock};
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
                .is_some_and(|id| !create_simulation::valid_id(id))
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
            || !create_simulation::valid_id(id)
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
            .all(|(id, n)| *n > 0 && create_simulation::valid_id(id))
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
    for s in slots[range].iter().flatten() {
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
/// Preserve components on every surviving input, including retained recipe catalysts.
fn consume(slots: &mut [Option<Stack>], remaining: &BTreeMap<String, u32>) {
    let mut remaining = remaining.clone();
    for slot in slots {
        if let Some(s) = slot {
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
        create_simulation::catalog::recipes(),
        limit,
    )
}
pub fn process_with_catalog(
    state: &MachineState,
    slots: &[Option<Stack>],
    station: &str,
    rpm: f32,
    heat: Heat,
    recipes: &[create_simulation::Recipe],
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
    // Fall back using the original state, so a press recipe's progress is not
    // reset by a failed compacting match on every tick over a basin.
    if result == TickResult::Idle && station == "compacting" {
        return process_with_catalog(state, slots, "pressing", rpm, heat, recipes, limit);
    }
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
mod tests;

pub mod worldgen;

/// A received rotation phase advanced by elapsed local time, independent of
/// periodic corrections to the client's world clock. Heartbeats arrive at 5 ticks;
/// stop predicting after 10 ticks rather than spin forever after a disconnect.
#[derive(Clone, Copy, Debug)]
pub struct RotationSample {
    pub angle: f32,
    pub rpm: f32,
}
impl RotationSample {
    pub fn capture(state: &MachineState, server_tick: u64) -> Self {
        Self {
            angle: state.render_angle(server_tick, 0.0),
            rpm: state.rpm,
        }
    }
    pub fn angle_after(&self, seconds: f64) -> f32 {
        let seconds = if seconds.is_finite() {
            seconds.clamp(0.0, 0.5)
        } else {
            0.0
        };
        (f64::from(self.angle) + seconds * f64::from(self.rpm) * 6.0).rem_euclid(360.0) as f32
    }
}
/// The local machine inventory takes precedence over an external depot/basin.
pub fn has_inputs(slots: &[Option<Stack>]) -> bool {
    slots.iter().take(INPUT.end).flatten().any(|s| s.count > 0)
}
/// Visible reasons a processing machine can be waiting.
pub fn status_label(state: &MachineState) -> &'static str {
    match state.status.as_str() {
        "Working" => "Processing",
        "Completed" => "Finished",
        "OutputBlocked" => "Output full",
        "NoPower" | "Unpowered" => "Needs rotation",
        "TooSlow" => "Needs 32+ RPM",
        "Cold" => "Needs burner heat",
        "NoMedium" => "Needs water/fire/lava",
        "Idle" => "No matching recipe",
        "Overstressed" => "Overstressed",
        "Conflict" | "Conflicting" | "Jammed" => "Conflicting drives",
        _ => "Ready",
    }
}
