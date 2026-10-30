//! Uses the pinned Horizon package parser, without prepared Minecraft resources.
use horizon_mods::{Package, content::Content};
fn main() {
    let path = std::path::PathBuf::from(std::env::args().nth(1).expect("hmod path"));
    let package = Package::read(&path).expect("valid hmod manifest");
    let content = Content::read(&path, &package.manifest).expect("valid content declarations");
    assert_eq!(content.blocks.len(), 34);
    assert_eq!(content.items.len(), 25);
    let recipes = content
        .files
        .iter()
        .filter(|p| p.contains("/recipe/"))
        .count();
    assert_eq!(recipes, 41);
    println!(
        "PASS: native content parsed by Horizon: {} blocks, {} separate items, {} recipes",
        content.blocks.len(),
        content.items.len(),
        recipes
    );
}
