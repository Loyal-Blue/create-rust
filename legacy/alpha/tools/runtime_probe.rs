//! Extra loaded/unloaded-chunk check using the real Horizon runtime.
//! Copy to crates/horizon-mods/examples/create_runtime_probe.rs in the
//! pinned Horizon tree, then run:
//! cargo run -p horizon-mods --example create_runtime_probe --features runtime -- /absolute/path/create_logic.hmod
use horizon_mods::{GameMode, Mods, Output, ServerPlayer, Status, WorldView};
use std::path::PathBuf;

struct View(bool);
impl WorldView for View {
    fn block(&self, dim: &str, x: i32, y: i32, z: i32) -> Option<String> {
        if !self.0 || dim != "minecraft:overworld" {
            return None;
        }
        Some(
            match (x, y, z) {
                (0, 64, 0) => "create:creative_motor[axis=x]",
                (1, 64, 0) => "create:shaft[axis=x]",
                _ => "minecraft:air",
            }
            .into(),
        )
    }
    fn biome(&self, _: &str, _: i32, _: i32, _: i32) -> Option<String> {
        self.0.then(|| "minecraft:plains".into())
    }
    fn has_dimension(&self, d: &str) -> bool {
        d == "minecraft:overworld"
    }
    fn check_state(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}
fn feedback(mods: &mut Mods, p: &ServerPlayer, cmd: &str) -> (String, bool) {
    assert!(mods.server_command(p, cmd));
    mods.drain()
        .into_iter()
        .find_map(|o| match o {
            Output::Reply { text, error, .. } => Some((text, error)),
            _ => None,
        })
        .expect("no feedback")
}
fn main() {
    let original = PathBuf::from(std::env::args().nth(1).expect("hmod path"));
    let temp = std::env::temp_dir().join(format!("create-runtime-probe-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let copy = temp.join("create_logic.hmod");
    std::fs::copy(original, &copy).unwrap();
    let mut mods = Mods::load(&[copy], |_| true, |_| false);
    assert_eq!(mods.entries()[0].status, Status::Running);
    let player = ServerPlayer {
        id: "probe".into(),
        name: "Probe".into(),
        x: 0.5,
        y: 64.0,
        z: 0.5,
        dimension: "minecraft:overworld".into(),
        game_mode: GameMode::Creative,
        health: 20.0,
    };
    mods.lend_world(Box::new(View(true)));
    feedback(&mut mods, &player, "kinetics info 0 64 0");
    let (text, error) = feedback(&mut mods, &player, "kinetics info 1 64 0");
    assert!(!error && text.contains("256 RPM; Running"));
    mods.lend_world(Box::new(View(false)));
    mods.server_tick(1);
    let (text, error) = feedback(&mut mods, &player, "kinetics info 1 64 0");
    assert!(error && text.contains("No loaded kinetic block"));
    mods.lend_world(Box::new(View(true)));
    mods.server_tick(2);
    let (text, error) = feedback(&mut mods, &player, "kinetics info 1 64 0");
    assert!(!error && text.contains("256 RPM; Running") && text.contains("2 blocks"));
    assert_eq!(mods.entries()[0].status, Status::Running);
    drop(mods);
    std::fs::remove_dir_all(temp).unwrap();
    println!(
        "PASS: unavailable chunks stop contributing; remembered networks resume without rescanning"
    );
}
