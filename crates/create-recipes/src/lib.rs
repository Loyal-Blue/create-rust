//! A recipe laboratory, intentionally separate from player inventories.
//! Recipes and RNG are not claimed to be byte-for-byte Java parity. Items
//! here have IDs and counts; NBT/components and world automation need host APIs.
use std::collections::BTreeMap;

pub mod catalog;

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

/// Select a recipe without consuming stock or advancing its random stream.
pub fn matching_recipe<'a>(
    stock: &Stock,
    station: &str,
    recipes: &'a [Recipe],
) -> Option<&'a Recipe> {
    recipes
        .iter()
        .find(|r| r.station == station && reserve(stock, r).is_some())
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
        self.tick_with(rpm, active_recipes())
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
            (self.progress + (rpm.abs() / 8.0).clamp(0.125, 32.0)).min(recipe.work.max(1) as f32);
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
        if !active_recipes().iter().any(|r| r.station == *station) {
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
                        && active_recipes()
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
mod tests;

mod containers;
/// Imported Create recipes plus authored generic fluid-container handlers.
pub fn active_recipes() -> &'static [Recipe] {
    static ALL: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        catalog::recipes()
            .iter()
            .chain(containers::recipes())
            .copied()
            .collect()
    })
    .as_slice()
}
