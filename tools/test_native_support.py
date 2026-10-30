#!/usr/bin/env python3
"""Compile support/migration fixtures against a local pinned Horizon checkout."""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--horizon',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1];n=r/'native/mixins/create_preview_support'
with tempfile.TemporaryDirectory(prefix='create-support-tests-') as tmp:
 t=Path(tmp);(t/'src').mkdir();(t/'Cargo.toml').write_text('[package]\nname="create-native-support-tests"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nhorizon-world={path='+json.dumps(str(a.horizon.resolve()/'crates/horizon-world'))+'}\nserde={version="1",features=["derive"]}\nserde_json="1"\n')
 source='pub type Result<T>=std::result::Result<T,Box<dyn std::error::Error>>;\npub use horizon_world::{registry,chunk};\nmod support { include!('+json.dumps(str(n/'support.rs'))+'); }\nmod migration {use std::io;use horizon_world::registry::BlockStateId;\n'+(n/'migrate.rs').read_text()+'\n#[cfg(test)]mod tests{'+(r/'tests/native_migration.rs').read_text()+'}}\n'
 (t/'src/lib.rs').write_text(source);subprocess.run(['cargo','test','--manifest-path',str(t/'Cargo.toml')],check=True)
