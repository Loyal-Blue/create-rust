# Create Rust for Horizon

An independent, unfinished Create-style mod for Horizon Minecraft 26.3. Not affiliated with the Create Team or Horizon.

## Current build: content 0.5.1 + native support 0.3.1

This build connects machinery to server ticks, real inventory slots, chunk saves and client rotation updates. It includes 36 blocks and 36 standalone items. It is an experimental machinery baseline, not a complete port of Create. No running-game verification has been performed.

- Directional placement, neighbor shaft alignment, upright press/mixer placement, wrench rotation, migrated saved states and corrected directional model assemblies.
- Create category on creative inventory page **2**, with page controls above the inventory.
- Water wheels, configurable creative motors and hand cranks; shafts, cog ratios, gearboxes, chain drives, clutches and gearshifts; stress limits and conflicting-source stops.
- Transactional pressing, mixing, compacting, milling, cutting, crushing, fan processing, deploying recipes, filling and draining, when their inputs/outputs exist in the installed registry.
- Burner fuel/heat, fluid storage and straight pipe pumping, chute/hopper integration, block drilling, and zinc in fresh overworld terrain.
- Machine progress, fuel, fluids, RPM and phase saved with chunks; existing container storage saves the items. Server snapshots drive client rotation.

See [functional scope](docs/FUNCTIONAL_SCOPE.md) for per-system behavior and gaps. Belts, contraptions, trains, full world deployer/saw behavior, multiblock tanks and exact Create GUIs are still absent. The 338-entry recipe catalogue is broader than the installed content; unavailable outputs are filtered before processing.

## Fixes in native 0.3.1

Machine-owned inputs now take priority over a depot/basin underneath. A press over a basin can also press a single ingot when no compacting recipe matches. Mixers can use their own item inputs with fluid from the basin below. Ingredient item IDs match even when stacks carry component data; surviving inputs and retained tools keep their components.

Rotation changes send on the next machine tick, with full snapshots every five ticks. Clients cache the snapshots and advance phase using elapsed local time instead of the periodically corrected world clock. Remote container status now follows the clicked machine and shows RPM, recipe progress, missing power/heat, insufficient mixer speed or a full output row. Prototype recipe work runs four times faster: an iron sheet takes about 2.5 seconds at 16 RPM or 5 seconds at 8 RPM, at 20 ticks/second.

## Installation and controls

Horizon standard content files cannot execute machinery code. Install both the content `.hmod` and experimental native `.hnmod`; the data-only pack remains static on stock Horizon.

1. Put `create-horizon-content-preview.hmod` in the instance's `horizon-mods/`.
2. Put `create-preview-support.hnmod` in the same instance's `horizon-native-mods/`.
3. Set `mod.create:true` and `mod.create_preview_support:true` in `horizon-mods.txt`.
4. Use **Native Mods → Build**, review the launcher's native-code notice, and launch that build. Restart fully after replacing content. Replace older copies rather than enabling duplicate mod IDs.

Native support pins Horizon `47a3b39fa9e419ec03221fca5ef402d58e831f14` (26.3) and horizon-mixins `cb1ad23022392d88f006cf94cbf6552a73f5df12`. Other native bundles may conflict.

Right-click with a wrench to rotate; sneak reverses rotation order. Empty-hand right-click on a motor cycles 0/8/16/32/64/128/256 RPM; sneak reverses its drive. Click a crank for one second of drive. Empty-hand clicks toggle clutches or reverse gearshifts; redstone also controls them. Water-wheel rim sampling uses radius 1 for small wheels and 2 for large wheels.

Open machine containers to put inputs in the **top row** and collect outputs from the **middle row**. The bottom row is spare storage. Presses and mixers can use a depot/basin one or two blocks below; a press over a basin compacts. Put a fueled blaze burner below the basin for heated mixing. Spouts can fill items on a depot directly below. Coal/charcoal, blaze rods and blaze cakes fuel burners; cakes superheat. Buckets insert/extract fluids from tanks, basins, spouts and drains. A pump transfers along its shaft axis through up to 16 straight pipe blocks; reverse RPM to reverse flow. Sneak-right-click reports status.

## Local asset build

Original Create textures/models are **All Rights Reserved**, per upstream [LICENSE.md](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/LICENSE.md). They are not committed. Import your private preview assets locally:

```sh
python3 tools/build_content.py --assets-from /path/to/current-private-preview.hmod --output dist/create.hmod
python3 tools/build_native.py --output dist/create-preview-support.hnmod
```

When upgrading older local assets, use `import_recipe_items.py` and `add_zinc_resources.py` with `--jar /path/to/your-create.jar --assets-from previous.hmod --output next.hmod`, then `prepare_world_models.py --assets-from next.hmod --output corrected.hmod` before the content build. Model preparation requires NumPy and SciPy. The original 1.21.1 Create 6.0.10 JAR is an asset input, not executable code for Horizon. Do not redistribute imported original art without appropriate permission. A public replacement asset set is still needed for an independently redistributable playable release. No Mojang resources are included.

## Source and checks

The root workspace builds placement (`create-core`), transmission (`create-transmission`), recipes (`create-recipes`) and inventory transactions/persistence (`create-machines`). `tools/build_native.py` embeds those sources into the self-contained native runtime. Edit the crates and `runtime_body.rs`/`machines_body.rs`, not generated `runtime.rs`. `legacy/` retains earlier experiments outside the active workspace.

```sh
cargo fmt --all --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
python3 tools/test_native_support.py --horizon /path/to/pinned/horizon
```

See [validation](docs/VALIDATION.md) for checks and their limits, and [standard API requirements](docs/ENGINE_REQUIREMENTS.md) for a future non-native implementation.
