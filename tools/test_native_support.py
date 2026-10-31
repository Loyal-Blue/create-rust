#!/usr/bin/env python3
"""Compile support/migration and real host ItemStack bridge fixtures."""
import argparse
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--horizon', type=Path, required=True)
a = p.parse_args()
r = Path(__file__).resolve().parents[1]
n = r / 'native/mixins/create_preview_support'
with tempfile.TemporaryDirectory(prefix='create-support-tests-') as tmp:
    t = Path(tmp)
    (t / 'src').mkdir()
    dependencies = {
        'horizon-world': a.horizon.resolve() / 'crates/horizon-world',
        'horizon-core': a.horizon.resolve() / 'crates/horizon-core',
        'horizon-items': a.horizon.resolve() / 'crates/horizon-items',
        'horizon-client-core': a.horizon.resolve() / 'crates/horizon-client-core',
        'create-runtime': r / 'crates/create-machines',
        'create-simulation': r / 'crates/create-recipes',
    }
    manifest = '[package]\nname="create-native-support-tests"\nversion="0.1.0"\nedition="2024"\n[dependencies]\n'
    manifest += ''.join(f'{name}={{path={json.dumps(str(path))}}}\n' for name, path in dependencies.items())
    manifest += 'serde={version="1",features=["derive"]}\nserde_json="1"\n'
    (t / 'Cargo.toml').write_text(manifest)
    body = (n / 'machines_body.rs').read_text()
    bridge = body[body.index('fn portable('):body.index('fn ensure_machine(')]
    source = 'pub type Result<T>=std::result::Result<T,Box<dyn std::error::Error>>;\npub use horizon_world::{registry,chunk};\n'
    source += 'mod support { include!(' + json.dumps(str(n / 'support.rs')) + ');\n'
    source += '#[cfg(test)] mod authored { use super::*; #[test] fn current_content_shapes_and_items_validate() { let config:Config=serde_json::from_str(include_str!(' + json.dumps(str(r / 'content/data/create/create_preview/support.json')) + ')).unwrap(); validate(&config).unwrap(); assert!(config.blocks.contains_key("create:belt")); assert!(config.blocks.contains_key("create:mechanical_crafter")); assert!(config.items.iter().any(|i|i.id=="create:mechanical_crafter")); assert!(config.items.iter().any(|i|i.id=="create:belt_connector")); let ids:std::collections::BTreeSet<_>=config.items.iter().map(|i|&i.id).collect(); assert_eq!(ids.len(),config.items.len()); }} }\n'
    source += 'mod migration {use std::io;use horizon_world::registry::BlockStateId;\n'
    source += (n / 'migrate.rs').read_text() + '\n#[cfg(test)]mod tests{' + (r / 'tests/native_migration.rs').read_text() + '}}\n'
    source += '#[cfg(test)] mod bridge { use create_runtime as machines; use create_simulation as recipes; use horizon_items::items::ItemStack;\n'
    source += 'mod horizon_client {pub use horizon_items::item_data;pub use horizon_client_core::snbt;}\n'
    bridge_visible = bridge.replace('fn portable(', 'pub(super) fn portable(').replace('fn restore(', 'pub(super) fn restore(')
    source += bridge_visible + (n / 'surface_items.rs').read_text() + '\nmod tests {\n' + (r / 'tests/native_machine_bridge.rs').read_text() + '\n}}\n'
    # Compile the production application adapter against a minimal isolated host
    # model, using real Horizon stacks and the same surface commit function.
    surface_body = (n / 'surface_body.rs').read_text()
    commit = surface_body[surface_body.index('fn commit_surface_stack('):surface_body.index('fn tick_depots(')]
    source += '#[cfg(test)] mod application_host {use create_runtime as machines;use create_simulation as recipes;use super::bridge::{portable,restore};use horizon_items::items::ItemStack;mod horizon_client {pub use horizon_items::item_data;}\n'
    source += (r / 'tests/deployer_host.rs').read_text() + '\n' + commit + (n / 'assembly_items.rs').read_text() + (n / 'applying_body.rs').read_text() + (r / 'tests/assembly_host.rs').read_text() + '\n}\n'
    assets = t / 'food-assets'
    (assets / 'data/minecraft/loot_table').mkdir(parents=True)
    for kind in ['block', 'item', 'entity_type']:
        (assets / f'data/minecraft/tags/{kind}').mkdir(parents=True)
    food_items = {}
    for name in ['bar_of_chocolate', 'honeyed_apple', 'chocolate_glazed_berries', 'builders_tea']:
        definition = json.loads((r / f'content/data/create/horizon/item/{name}.json').read_text())
        food_items[f'create:{name}'] = dict(definition['components'], **{'minecraft:max_stack_size': 64})
    for name in ['precision_mechanism', 'incomplete_precision_mechanism']:
        definition=json.loads((r/f'content/data/create/horizon/item/{name}.json').read_text())
        food_items[f'create:{name}']={'minecraft:max_stack_size':definition.get('max_stack_size',64)}
    (assets / 'items.json').write_text(json.dumps(food_items))
    source += '#[cfg(test)] mod food { #[test] fn authored_food_and_assembly_components_load_through_horizon() { let items=horizon_items::items::Items::catalog(std::path::Path::new(' + json.dumps(str(assets)) + '),&horizon_world::loot::PackOverlay::NONE).unwrap(); let tea=items.get("create:builders_tea").unwrap(); assert!(tea.can_always_eat); assert_eq!(tea.use_remainder.as_deref(),Some("minecraft:glass_bottle"));assert_eq!(items.get("create:incomplete_precision_mechanism").unwrap().max_stack,1);assert_eq!(items.get("create:precision_mechanism").unwrap().max_stack,64); assert_eq!(tea.consumable.as_ref().unwrap().seconds,2.0); assert!(matches!(tea.consumable.as_ref().unwrap().animation,horizon_world::consumable::UseAnimation::Drink)); let effects=&tea.consumable.as_ref().unwrap().effects; assert!(effects.iter().any(|e|matches!(e,horizon_items::items::ConsumeEffect::ApplyEffects{effects,probability} if *probability==1.0 && effects.iter().any(|e|e.id=="minecraft:haste" && e.duration==3600 && e.amplifier==0)))); assert_eq!(items.get("create:bar_of_chocolate").unwrap().food,Some((6,7.2))); assert_eq!(items.get("create:honeyed_apple").unwrap().food,Some((8,9.6))); assert_eq!(items.get("create:chocolate_glazed_berries").unwrap().food,Some((6,7.2))); }}\n'
    (t / 'src/lib.rs').write_text(source)
    subprocess.run(['cargo', 'test', '--manifest-path', str(t / 'Cargo.toml')], check=True)
