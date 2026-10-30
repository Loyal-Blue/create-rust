"""Self-contained Rust mixin generation from the tested project crates."""
from pathlib import Path

def source(path):return path.read_text().replace('//!','//')
def generate(root):
 c=root/'crates';m=root/'native/mixins/create_preview_support'
 transmission=source(c/'create-transmission/src/lib.rs').replace('pub mod state;','pub mod state {\n'+source(c/'create-transmission/src/state.rs').replace('use crate::{','use super::{')+'\n}').replace('mod tests;','mod tests {\n'+source(c/'create-transmission/src/tests.rs')+'\n}')
 recipes=source(c/'create-recipes/src/lib.rs').replace('pub mod catalog;','pub mod catalog {\n'+source(c/'create-recipes/src/catalog.rs').replace('use crate::{','use super::{')+'\n}').replace('mod tests;','mod tests {\n'+source(c/'create-recipes/src/tests.rs')+'\n}')
 machines=source(c/'create-machines/src/lib.rs').replace('pub mod worldgen;','pub mod worldgen {\n'+source(c/'create-machines/src/worldgen.rs')+'\n}').replace('pub use create_kinetics as kinetics;','pub use super::transmission as kinetics;').replace('create_simulation::','super::recipes::').replace('mod tests;','mod tests {\n'+source(c/'create-machines/src/tests.rs')+'\n}')
 body='#[allow(dead_code)]\nmod kinetic {\n'+source(c/'create-core/src/lib.rs')+'\n}\n#[allow(dead_code)]\npub mod transmission {\n'+transmission+'\n}\n#[allow(dead_code)]\npub mod recipes {\n'+recipes+'\n}\n#[allow(dead_code)]\npub mod machines {\n'+machines+'\n}\n'+source(m/'runtime_body.rs')+'\n'+source(m/'machines_body.rs')
 (m/'runtime.rs').write_text(body)
