# Native Create development checkpoint — 0.3.0

This is an unfinished Rust port of Create 6.0.10 for Horizon's Minecraft 26.3 engine. The new native engine patch replaces the earlier virtual laboratory with real machine inventories. It is source for a custom Horizon build, not a finished full-Create release or a drop-in code mod for the stock launcher.

The legacy `dist/create.hmod` and `dist/create_logic.hmod` binaries remain the historical 0.2 alpha. They have not been rebuilt for the native code. Do not install the legacy logic alongside the native integration. The new `dist/create-native-content.hmod` is content only and requires the matching patched engine on both host and clients.

## Build

Requires Git, Python 3.10+, and Rust 1.99.1 or newer. Start with the exact tested Horizon source revision:

```sh
git clone https://github.com/horizon-rust/horizon.git horizon
cd horizon
git checkout --detach 87a6a7a161814ffffc846950aba4997f592df4fc
cd ../create-horizon-26.3
./build-native.sh /absolute/path/to/horizon
```

The script checks the revision, applies `engine/native-create.patch`, copies the three Rust crates under `mods/create/`, generates the original content package, runs the core tests and save test, then builds the engine with `--features create`. Native Create is off by default. The script builds without audio/controllers to avoid Linux ALSA/udev dependencies; add the normal default features when building if desired.

Applying the patch again is allowed when it already matches exactly. Conflicting engine edits are rejected. This patch is pinned; automatic adaptation to later Horizon revisions is not implemented.

Use Horizon's resource preparation instructions with your own Minecraft 26.3 installation. No Minecraft game files or prepared resources are included. Copy only `dist/create-native-content.hmod` to the test game's `horizon-mods/` directory and enable that content mod. Run the patched engine, use a fresh Creative test world, and keep identical engine/content versions on every participant.

## Implemented native code

- Actual 27-slot machine containers: inputs 0–8, outputs 9–17, utility storage 18–26. Real ItemStack components survive moves; componentful ingredients are deliberately left untouched by the ID/count recipe matcher.
- Server tick kinetics for sources, shafts, cog ratios, gearboxes, chain drives, clutches and gearshifts. Cranks run while used; clutch/gearshift state responds to real redstone. Wheels use a simplified adjacent-water check.
- Processing adapters for milling, pressing, basin compacting/mixing, cutting, crushing, deploying, fan splashing, filling and emptying, using the existing supported recipe catalog. Missing host output IDs stop execution safely.
- Presses and mixers locate a basin/depot one or two blocks below. Burner fuel timers supply heated/superheated recipe checks. Fire/lava/campfires use a simplified heat rule.
- Bucket insertion/extraction for tanks, basins, spouts and drains, with real survival bucket exchange. Capacity is 8000 mB per input/output tank, not the final Create tank multiblock system.
- Powered pumps search connected pipe blocks up to 16 steps and move a conserved volume with one total RPM-based budget per pump/tick. Basic chutes move real items downward with an eight-tick cooldown.
- Hopper insertion into processing inputs and extraction from processing outputs. Generic machine storage uses Horizon's existing container/menu infrastructure.
- Native block-entity item/state save/load, chunk dirty tracking, and network update payloads. A chest protocol carrier plus native ID marker transports custom state between patched peers.
- Rotor submission for shafts, cogwheels, water wheels and cranks. RPM changes preserve rotation phase across saves. Original static meshes are hidden for these rotors in the native content package.

## Important gaps

Full Create parity is not complete. There are no moving contraptions, bearings/gantries/pulleys, trains/rail networks, schematics, Ponder, working belt transport, mechanical crafting networks, storage multiblocks, full fluid pressure/valves/hoses, or Create's full tools/equipment and world-generation systems. The `Belt` kinetic kind is only a shaft-like network node; no belt block is exposed by this content build.

Processing has approximate timings and deterministic Rust RNG, not Java-identical timing/random streams. Tool wear, component predicates, sequenced assembly and many recipe serializers remain missing. Fan effects do not trace airflow or process world entities. Saw/drill/deployer world actions and crushing-wheel entity interactions are unfinished. Native menus use a generic chest layout and permit manual output-slot edits. Basin fluid tanks are simplified. Wheel size/flow logic, all collision shapes and several new block models are development approximations.

The native Linux client compiles and links successfully. This environment has no prepared player-owned Minecraft resources, so live gameplay, rendered appearance, survival progression and real multiplayer sessions are not verified. Consult `NATIVE_VALIDATION.md` for exact checks performed; source compilation does not establish gameplay parity.

## Manual acceptance checks still required

1. Start the normal integrated server thread with the patched engine and content enabled. Place a motor, axis-aligned shaft and press. Open the press, insert ordinary iron ingots in the first row and observe real iron sheets in the second row.
2. Save/reopen mid-recipe. Confirm input/output counts, progress, tank contents and crank/fuel timers persist without duplication.
3. Connect hoppers to input and output faces. Test full outputs, disconnected power, overstress and redstone clutch/gearshift changes.
4. Test two patched network peers opening the same machine, moving items, changing RPM and reconnecting. Confirm synchronized items and state.
5. Test bucket exchanges and pump movement at full/empty tanks. Verify total fluid conservation and no discarded survival buckets with a full inventory.
6. Visually inspect rotor centering, orientation, lighting and animation at each axis and chunk boundary. Check the inherited full-cube collisions separately.

Use disposable test data until those checks pass. Game resources and player saves are intentionally not part of this package.
