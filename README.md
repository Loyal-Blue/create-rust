# Create Rust for Horizon

An independent, unfinished Create-style mod for Horizon Minecraft 26.3. Not affiliated with the Create Team or Horizon.

## Current build: content 0.4.0 + native support 0.2.0

Horizon standard content files cannot execute machinery code. Use the content `.hmod` and the experimental native `.hnmod` together. Native support is allowed for this project; the data-only pack remains a static preview on stock Horizon.

Implemented in the new native source:

- Facing states for motors and directional machines; six-direction placement for directional parts, horizontal orientation for upright machines, shaft alignment and a sneak bypass.
- Wrench rotation (sneak reverses), and migration of older axis/waterlogged preview states for changed machines.
- Create category on creative inventory page **2**, with `< 1 / 2 >` page controls above the inventory. Vanilla categories stay on page 1; search and inventory remain accessible.
- Dynamic wheel and shaft meshes driven by a basic client-side coaxial network. Flowing-water torque drives wheels at 8 RPM; a creative motor drives at fixed 16 RPM. Disconnected shafts stop; conflicting sources jam. This first step is visual rotation, not an authoritative processing system.

**Experimental: not tested in a running game.** Engine compilation and automated fixtures pass; those checks do not prove placement/rendering parity. Geometry and placement rules are approximations of static models. No full wheel clearance/multiblock structure checks, gears, stress, machine processing, belts, contraptions or trains are claimed.

## Installation

1. Put content `create.hmod` in the instance's `horizon-mods/`.
2. Put `create-preview-support.hnmod` in the same instance's `horizon-native-mods/`.
3. In `horizon-mods.txt`, set `mod.create:true` and `mod.create_preview_support:true`.
4. Use the launcher's **Native Mods → Build**, review its native-code notice, and launch that build. Restart fully after replacing content.

The native bundle pins Horizon `47a3b39fa9e419ec03221fca5ef402d58e831f14` (26.3) and horizon-mixins `cb1ad23022392d88f006cf94cbf6552a73f5df12`. Compatibility with every other native bundle is not guaranteed.

Right-click a Create block with the wrench to rotate it. Use the page arrows above creative inventory to reach Create on page 2. For a first rotation test, place a horizontal wheel with flowing water along its rim and coaxial shafts touching the center. The sampled rim is one block from a small wheel's center and two from a large wheel's center. Creative motors supply a fixed prototype speed; there is no speed GUI yet.

## Build

Original Create textures/models are **All Rights Reserved**, per the upstream [LICENSE.md](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/LICENSE.md). They are not committed. Import assets from your existing private preview locally:

```sh
python3 tools/build_content.py --assets-from /path/to/create-horizon-content-preview.hmod --output dist/create.hmod
python3 tools/build_native.py --output dist/create-preview-support.hnmod
```

The content builder takes the authored declarations from this repository and model/texture inputs from the private archive. Do not redistribute imported original art without appropriate permission. A public asset replacement set remains needed for a fully redistributable playable release. Neither the original JAR nor Mojang game assets are included.

## Source and tests

`crates/create-core` is the engine-independent placement/rotation core. `native/mixins/create_preview_support` connects it to the current engine; `tools/build_native.py` embeds the core into a self-contained native helper. `legacy/` preserves earlier alpha/runtime/SDK/native experiments without building them from the root workspace.

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
python3 tools/test_native_support.py --horizon /path/to/pinned/horizon
```

The tests cover placement orientations, shaft alignment, water torque, topology changes, jams, phase integration, save migration, inventory-page visibility and safe resource overlays. See `docs/VALIDATION.md` for exactly what was checked. Horizon's standard API gaps remain documented in `docs/ENGINE_REQUIREMENTS.md` for a future standard-mod implementation.
