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
        'horizon-items': a.horizon.resolve() / 'crates/horizon-items',
        'horizon-client-core': a.horizon.resolve() / 'crates/horizon-client-core',
        'create-runtime': r / 'crates/create-machines',
    }
    manifest = '[package]\nname="create-native-support-tests"\nversion="0.1.0"\nedition="2024"\n[dependencies]\n'
    manifest += ''.join(f'{name}={{path={json.dumps(str(path))}}}\n' for name, path in dependencies.items())
    manifest += 'serde={version="1",features=["derive"]}\nserde_json="1"\n'
    (t / 'Cargo.toml').write_text(manifest)
    body = (n / 'machines_body.rs').read_text()
    bridge = body[body.index('fn portable('):body.index('fn ensure_machine(')]
    source = 'pub type Result<T>=std::result::Result<T,Box<dyn std::error::Error>>;\npub use horizon_world::{registry,chunk};\n'
    source += 'mod support { include!(' + json.dumps(str(n / 'support.rs')) + '); }\n'
    source += 'mod migration {use std::io;use horizon_world::registry::BlockStateId;\n'
    source += (n / 'migrate.rs').read_text() + '\n#[cfg(test)]mod tests{' + (r / 'tests/native_migration.rs').read_text() + '}}\n'
    source += '#[cfg(test)] mod bridge { use create_runtime as machines; use horizon_items::items::ItemStack;\n'
    source += 'mod horizon_client {pub use horizon_items::item_data;pub use horizon_client_core::snbt;}\n'
    source += bridge + '\nmod tests {\n' + (r / 'tests/native_machine_bridge.rs').read_text() + '\n}}\n'
    (t / 'src/lib.rs').write_text(source)
    subprocess.run(['cargo', 'test', '--manifest-path', str(t / 'Cargo.toml')], check=True)
