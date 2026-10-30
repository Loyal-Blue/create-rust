// Geometry for the static Create content preview; no machine simulation.
pub static PAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
use horizon_world::{chunk::BlockState, nbt::{self, Tag}};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

pub fn page_visible(id: &str, search: bool, inventory: bool, page: usize) -> bool {
    if id == "create:main" { page == 1 } else { page == 0 || search || inventory }
}
type Boxes = Vec<[f64; 6]>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: u32,
    blocks: BTreeMap<String, BTreeMap<String, Boxes>>,
    items: Vec<TabItem>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TabItem { id: String, name: String }

fn validate(config: &Config) -> crate::Result<()> {
    if config.schema != 1 || config.blocks.len() < 34 || config.blocks.len() > 4096 || config.items.is_empty() || config.items.len() > 4096 || config.blocks.get("create:creative_motor").is_none_or(|v| !v.contains_key("north")) {
        return Err("Create support needs the matching 0.6.x content preview".into());
    }
    for variants in config.blocks.values() {
        for boxes in variants.values() {
            if boxes.is_empty() || boxes.len() > 256 || boxes.iter().any(|b| {
                b.iter().any(|n| !n.is_finite() || *n < 0.0 || *n > 1.0)
                    || (0..3).any(|a| b[a] >= b[a+3])
            }) {
                return Err("Invalid Create preview collision boxes".into());
            }
        }
    }
    Ok(())
}

fn boxes_for<'a>(config: &'a Config, state: &BlockState) -> Option<&'a Boxes> {
    let variants = config.blocks.get(&state.name)?;
    let axis = state.properties.get("facing").or_else(|| state.properties.get("axis")).map(String::as_str).unwrap_or("");
    variants.get(axis).or_else(|| variants.get(""))
}

// A union of boxes becomes the same occupancy grid the engine normally exports.
fn grid(boxes: &Boxes) -> Tag {
    let coords: [Vec<f64>; 3] = std::array::from_fn(|a| {
        let mut values: Vec<_> = boxes.iter().flat_map(|b| [b[a], b[a+3]]).collect();
        values.sort_by(f64::total_cmp);
        values.dedup();
        values
    });
    let size = coords.each_ref().map(|v| v.len()-1);
    let mut occupied = Vec::new();
    let mut min = size.map(|n| n as i32);
    let mut max = [0; 3];
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let indexes = [x,y,z];
                let point: [f64; 3] = std::array::from_fn(|a| (coords[a][indexes[a]]+coords[a][indexes[a]+1])/2.0);
                let full = boxes.iter().any(|b| (0..3).all(|a| point[a] > b[a] && point[a] < b[a+3]));
                occupied.push(u8::from(full));
                if full {
                    for a in 0..3 { min[a] = min[a].min(indexes[a] as i32); max[a] = max[a].max(indexes[a] as i32+1); }
                }
            }
        }
    }
    Tag::compound([
        ("canonical", Tag::Byte(0)),
        ("x", Tag::LongArray(coords[0].iter().map(|n| n.to_bits() as i64).collect())),
        ("y", Tag::LongArray(coords[1].iter().map(|n| n.to_bits() as i64).collect())),
        ("z", Tag::LongArray(coords[2].iter().map(|n| n.to_bits() as i64).collect())),
        ("occupied", Tag::ByteArray(occupied)),
        ("cube", Tag::Byte(0)),
        ("cube_axes", Tag::Byte(0)),
        ("discrete_min", Tag::IntArray(min.to_vec())),
        ("discrete_max", Tag::IntArray(max.to_vec())),
    ])
}

fn collision(tag: &mut Tag, config: &Config) -> crate::Result<BTreeMap<usize, Boxes>> {
    let Some(Tag::List(states)) = tag.get("states") else { return Err("Missing collision states".into()); };
    let mut edits = BTreeMap::new();
    for (index, state) in states.iter().enumerate() {
        let state = BlockState::from_nbt(state)?;
        if let Some(boxes) = boxes_for(config, &state) { edits.insert(index, boxes.clone()); }
    }
    let Some(Tag::List(shapes)) = tag.get_mut("shapes") else { return Err("Missing collision shapes".into()); };
    let mut ids = BTreeMap::new();
    for (index, boxes) in &edits {
        let shape = grid(boxes);
        let id = shapes.iter().position(|s| *s == shape).unwrap_or_else(|| { shapes.push(shape); shapes.len()-1 });
        ids.insert(*index, id as i32);
    }
    let Some(Tag::IntArray(mapping)) = tag.get_mut("cached_shapes") else { return Err("Missing collision mapping".into()); };
    for (index, id) in ids { *mapping.get_mut(index).ok_or("Collision mapping too short")? = id; }
    Ok(edits)
}

fn edges(boxes: &Boxes) -> Boxes {
    let mut edges = Vec::new();
    for b in boxes {
        for a in 0..3 {
            let c = (a+1)%3; let d = (a+2)%3;
            for i in [c,c+3] { for j in [d,d+3] {
                let mut line = [0.0;6];
                line[a]=b[a]; line[a+3]=b[a+3];
                line[c]=b[i]; line[c+3]=b[i];
                line[d]=b[j]; line[d+3]=b[j];
                if !edges.contains(&line) { edges.push(line); }
            }}
        }
    }
    edges
}

fn creative(raw: &mut Value, config: &Config) -> crate::Result<()> {
    let tabs = raw["tabs"].as_array_mut().ok_or("Missing creative tabs")?;
    tabs.retain(|t| t["id"] != "create:main");
    let stacks: Vec<_> = config.items.iter().map(|i| format!("{{count:1,id:\"{}\"}}",i.id)).collect();
    for tab in tabs.iter_mut().filter(|t| t["type"]=="SEARCH") {
        let count = tab["items"].as_array().ok_or("Missing search items")?.len();
        for field in ["lines","tags"] {
            if !tab[field].is_array() { tab[field] = json!([]); }
            tab[field].as_array_mut().unwrap().resize_with(count,||json!([]));
        }
        for (item, stack) in config.items.iter().zip(&stacks) {
            if tab["items"].as_array().unwrap().iter().any(|s| s.as_str() == Some(stack)) { continue; }
            tab["items"].as_array_mut().unwrap().push(json!(stack));
            tab["lines"].as_array_mut().unwrap().push(json!([item.name,"Create"]));
            tab["tags"].as_array_mut().unwrap().push(json!([]));
        }
    }
    tabs.push(json!({"id":"create:main","title":"Create","icon":"{count:1,id:\"create:cogwheel\"}","type":"CATEGORY","row":"TOP","column":0,"visible":true,"items":stacks}));
    Ok(())
}

// The overlay may contain symlinks into vanilla resources. Replace the link
// itself using rename, rather than opening it for writing and changing vanilla.
fn replace(root: &Path, relative: &str, bytes: &[u8]) -> crate::Result<()> {
    let path = root.join(relative);
    let temp = path.with_extension("create-preview.tmp");
    let _ = fs::remove_file(&temp);
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
    std::io::Write::write_all(&mut file, bytes)?;
    drop(file);
    if let Err(error) = fs::rename(&temp, &path) {
        // Windows does not replace an existing destination with rename.
        if path.symlink_metadata().is_ok() { fs::remove_file(&path)?; fs::rename(&temp, &path)?; }
        else { return Err(error.into()); }
    }
    Ok(())
}

pub(super) fn apply(root: &Path) -> crate::Result<()> {
    let path = root.join("data/create/create_preview/support.json");
    if !path.exists() { return Err("Install Create content preview 0.6.x alongside Create Preview Support".into()); }
    let config: Config = serde_json::from_slice(&fs::read(path)?)?;
    validate(&config)?;
    let mut changed = BTreeMap::new();
    let mut state_names = BTreeMap::new();
    for relative in ["cached-collision.nbt","worldgen/blocks/cached-collision.nbt"] {
        let (mut named,_) = nbt::read_named(&fs::read(root.join(relative))?,nbt::Limits::tables())?;
        if relative=="cached-collision.nbt" {if let Some(Tag::List(states))=named.tag.get("states") {for (id,s) in states.iter().enumerate() {state_names.insert(id,BlockState::from_nbt(s)?.name);}}}
        let edits = collision(&mut named.tag, &config)?;
        if relative=="cached-collision.nbt" { changed=edits; }
        replace(root,relative,&nbt::write_named(&named,nbt::Limits::tables())?)?;
    }
    let mut facts: Value = serde_json::from_slice(&fs::read(root.join("gameplay-facts.json"))?)?;
    let states = facts["states"].as_array_mut().ok_or("Missing gameplay states")?;
    for (index, boxes) in changed {
        let row = states.get_mut(index).ok_or("Missing Create gameplay row")?;
        if row["id"].as_u64() != Some(index as u64) { return Err("Create gameplay state order mismatch".into()); }
        row["outline"]=json!(boxes);
        row["block_entity"]=json!(!matches!(state_names.get(&index).map(String::as_str),Some("create:andesite_alloy_block"|"create:brass_block"|"create:zinc_block"|"create:andesite_casing"|"create:brass_casing"|"create:copper_casing"|"create:zinc_ore"|"create:deepslate_zinc_ore")));
        row["outline_edges"]=json!(edges(&boxes));
        row["full_collision"]=json!(boxes.iter().any(|b| *b == [0.0,0.0,0.0,1.0,1.0,1.0]));
    }
    replace(root,"gameplay-facts.json",&serde_json::to_vec(&facts)?)?;
    // Hide static kinetic terrain only in the native overlay; the data-only
    // content archive keeps its normal static previews.
    replace(root,"assets/create/models/block/horizon_dynamic_empty.json",br#"{"elements":[],"textures":{"particle":"create:block/axis"}}"#)?;
    for name in ["shaft","water_wheel","large_water_wheel","cogwheel","large_cogwheel","crushing_wheel"] {
        let relative=format!("assets/create/blockstates/{name}.json");
        let mut states: Value=serde_json::from_slice(&fs::read(root.join(&relative))?)?;
        if let Some(variants)=states["variants"].as_object_mut() {
            for variant in variants.values_mut() {variant["model"]=json!("create:block/horizon_dynamic_empty");}
        }
        replace(root,&relative,&serde_json::to_vec(&states)?)?;
    }
    let mut tabs: Value = serde_json::from_slice(&fs::read(root.join("creative-inventory.json"))?)?;
    creative(&mut tabs,&config)?;
    replace(root,"creative-inventory.json",&serde_json::to_vec(&tabs)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_category_is_on_page_two_only() {
        assert!(!page_visible("create:main", false, false, 0));
        assert!(page_visible("create:main", false, false, 1));
        assert!(!page_visible("minecraft:building_blocks", false, false, 1));
        assert!(page_visible("minecraft:building_blocks", false, false, 0));
        assert!(page_visible("minecraft:search", true, false, 1));
        assert!(page_visible("minecraft:inventory", false, true, 1));
    }
    fn config() -> Config {
        Config { schema:1, blocks:BTreeMap::from([("create:creative_motor".into(),BTreeMap::from([("".into(),vec![[0.125,0.125,0.0,0.875,0.875,1.0]])]))]),items:(0..59).map(|i|TabItem{id:format!("create:item_{i}"),name:format!("Create Item {i}")}).collect() }
    }
    #[test]
    fn motor_body_replaces_chain_shape_and_preserves_vanilla() {
        let config=config();
        let states=vec![BlockState{name:"minecraft:stone".into(),properties:BTreeMap::new()}.to_nbt(),BlockState{name:"create:creative_motor".into(),properties:BTreeMap::new()}.to_nbt()];
        let mut table=Tag::compound([("states",Tag::List(states)),("shapes",Tag::List(vec![grid(&vec![[0.0,0.0,0.0,1.0,1.0,1.0]])])),("cached_shapes",Tag::IntArray(vec![0,0]))]);
        let changed=collision(&mut table,&config).unwrap();
        assert_eq!(changed.len(),1);
        assert_eq!(table.get("cached_shapes"),Some(&Tag::IntArray(vec![0,1])));
        let Some(Tag::List(shapes))=table.get("shapes") else { panic!() };
        assert_eq!(shapes[1].get("x"),Some(&Tag::LongArray(vec![0.125f64.to_bits() as i64,0.875f64.to_bits() as i64])));
        assert_eq!(shapes[1].get("occupied"),Some(&Tag::ByteArray(vec![1])));
    }
    #[test]
    fn create_tab_and_search_have_all_items_without_duplicates() {
        let config=config();let mut raw=json!({"tabs":[{"id":"minecraft:search","type":"SEARCH","items":["{count:1,id:\"minecraft:stone\"}"]}]});
        creative(&mut raw,&config).unwrap();creative(&mut raw,&config).unwrap();
        assert_eq!(raw["tabs"].as_array().unwrap().len(),2);
        assert_eq!(raw["tabs"][1]["items"].as_array().unwrap().len(),59);
        for field in ["items","lines","tags"] { assert_eq!(raw["tabs"][0][field].as_array().unwrap().len(),60); }
    }
    #[cfg(unix)]
    #[test]
    fn overlay_replacement_does_not_modify_symlink_target() {
        let root=std::env::temp_dir().join(format!("create-preview-test-{}",std::process::id()));fs::create_dir_all(&root).unwrap();
        fs::write(root.join("base.json"),b"vanilla").unwrap();
        std::os::unix::fs::symlink(root.join("base.json"),root.join("overlay.json")).unwrap();
        replace(&root,"overlay.json",b"create").unwrap();
        assert_eq!(fs::read(root.join("base.json")).unwrap(),b"vanilla");
        assert_eq!(fs::read(root.join("overlay.json")).unwrap(),b"create");
        fs::remove_dir_all(root).unwrap();
    }
}
