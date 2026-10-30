Current native development status is in [NATIVE_DEVELOPMENT.md](NATIVE_DEVELOPMENT.md), including the remaining full-mod work. The list below is the historical 0.2 alpha assessment and does not describe the new native inventory/save/network code.

---

# Remaining work — Create: Horizon 26.3 Alpha 0.2.0

This is a partial port with compiled packages and real sandbox verification.
Check this list before calling it a playable Create replacement.

## Completed in this package

- [x] Pin Horizon revision and exact Create JAR SHA-256 in `UPSTREAM.toml`.
- [x] Build actual WebAssembly against Horizon's real, vendored SDK/WIT.
- [x] Split API 0.2 content and API 0.3 server code into valid, distinct packages.
- [x] Package both `.hmod` files and pass Horizon's official package/sandbox checks.
- [x] Retain and extend the original kinetic network solver.
- [x] Add perpendicular bevel gears, explicit shaft ports, clutch and gearshift logic.
- [x] Select the driving source using network-relative gear ratios.
- [x] Revalidate remembered blocks; exclude unavailable chunks from active networks.
- [x] Replace synchronous large scans with a bounded queue.
- [x] Add 25 declared blocks, 21 items/parts, 32 crafting recipes and original shaped models.
- [x] Import the JAR's 1,884 unique recipe IDs as MIT reference data.
- [x] Interpret 338 processing recipes, including heat, fluid amounts, retained tools and chance outputs.
- [x] Implement a Creative-only virtual lab, independent per player and driven by solved RPM.
- [x] Reserve inputs/outputs atomically and prevent rerolling blocked chance results.
- [x] Persist virtual work/RNG/inventories and source controls under explicit world slots.
- [x] Add a repeatable build/validation script, installation steps and demo commands.

## 1. Verify in a real Horizon game

The sandbox does not load the renderer, vanilla registries or the world simulation.

- [ ] Enable both packages in a fresh prepared 26.3 test installation.
- [ ] Confirm all 46 IDs register, render, localize, place/give and save correctly.
- [ ] Check each axis and each model; verify intended collision and occlusion.
- [ ] Craft all 32 recipes and verify drops by hand, correct tool and explosions.
- [ ] Test default single player, `--in-frame`, LAN and a dedicated host.
- [ ] Verify content synchronization, world reload, mod disable/re-enable and removal.
- [ ] Measure tick time at the 512-block/16-lab limits and optimize or lower limits.

## 2. Extend or obtain the required Horizon host APIs

The target SDK exposes server player snapshots, commands, place/break events,
loaded-block reads/writes, storage and ticks. It does not expose the following
Create requirements. Implement these as versioned, capability-gated interfaces
in a Horizon fork before presenting them as ordinary supported mod behavior.

| Needed host API | Engine work | Acceptance test |
|---|---|---|
| Stable world UUID and lifecycle | Expose world identity, save, close and chunk load/unload | Two worlds with identical coordinates never share machine data; clean shutdown flushes state |
| Block entities | Persistent per-position data, ticking lifecycle and synchronization | Save/restart/reload retains a machine exactly once; unload pauses it |
| Item handlers | Component-aware read, insert/extract/simulate transactions; sided access | Two players/hoppers cannot duplicate or lose one stack |
| Fluid handlers | Typed fluid IDs/components, capacities, atomic transfers | Amount is conserved across tank, pipe and machine operations |
| Block interactions | Use/attack/tool and neighbour/redstone callbacks | Wrench rotates a machine, crank responds to use, clutch follows power |
| Server authorization | Operator level and protection hooks | Mod setup and world edits respect server permissions |
| Machine screens | Server-owned slots, progress/gauges, component transactions | Client clicks validate on server and synchronize under latency |
| Custom block states/shapes | Native blockstate definitions and per-state collision/occlusion | Shafts, basins and larger wheels have correct placement and collision |
| Client synchronization/rendering | Budgeted animated block/entity meshes and typed messages | Visible rotation follows authoritative RPM, axis and stop state |
| Entities/contraptions | Spawn, persistent entity data, movement, collisions and block access | A moved structure can assemble and disassemble without loss |

SDK entry points in the pinned Horizon tree: `crates/horizon-mod-sdk/wit/horizon.wit`,
`crates/horizon-mods/src/runtime.rs`, `crates/horizon-client/src/game/mods.rs` and
`crates/horizon-client/src/mod_content.rs`. See upstream `docs/mods.md` and
`docs/modpacks.md` for the contract and roadmap; some older roadmap tables are stale,
so actual WIT/source is authoritative.

## 3. Turn the laboratory into actual machines

- [ ] Replace each virtual stock with atomic real item/fluid handlers and block entities.
- [ ] Port stack components, durability, enchantment and recipe predicates.
- [ ] Port exact per-machine processing rates, speed thresholds, stress and RNG.
- [ ] Implement millstone, press, basin/mixer, saw, crushing pair and deployer behavior.
- [ ] Add actual targets: press/depot/belt, mixer/basin, fan airflow/media, deployer reach.
- [ ] Drill/saw world processing: permissions, hardness/tool rules, drops and block updates.
- [ ] Add spouts, drains, sanding and item application to in-game interaction.
- [ ] Drive heat from real burners and fuel; preserve fuel and fluid accounting.
- [ ] Test blocked outputs, interruption, simultaneous access, crash/restart and chunk boundaries.
- [ ] Remove virtual item creation from survival interfaces. Keep a clearly labelled debug lab.

## 4. Finish kinetic mechanics

- [ ] Exact ports/orientation for each block; correct single-ended sources and consumers.
- [ ] Gearboxes, belts, chain drives, encased chains, speed controllers and adjustable drives.
- [ ] Independent input/output halves for clutches and gearshifts; real redstone control.
- [ ] Water flow and wheel geometry, multiblock validation and rotation direction.
- [ ] Windmills and sail counting; steam engines, boilers, heat and water supply.
- [ ] Incremental network updates for large factories; proper source takeover and feedback.
- [ ] Differential tests against the original mod for topology, signs, speeds, capacity and stress.

## 5. Automation, moving structures and trains

- [ ] Belts, funnels, tunnels, chutes, filters, vaults, stock links and item logistics.
- [ ] Tanks, pipes, pumps, valves, tanks/chunk boundaries and fluid recipes.
- [ ] Mechanical/sequenced crafting and transitional items with weighted final results.
- [ ] Bearings, pistons, gantries, attached tools, collision and safe disassembly.
- [ ] Railway tracks, bogeys, trains, controls, schedules, signals and routing.
- [ ] Schematic capture, validation, placement and survival material accounting.
- [ ] Ponder scenes, goggles, wrench interactions, machine UI, particles, audio and advancements.
- [ ] Decorative block catalog and all remaining Create items/data.

## 6. Parity and release gate

- [ ] Account for each omitted recipe in `reference/import-report.json`; enable a recipe only
  when its dependencies/ingredients/serializer really work.
- [ ] Run an oracle against Create 6.0.10 to compare recipes, RNG, machine ticks and outputs.
- [ ] Confirm that build/package checks and every simulation test pass.
- [ ] Complete visual, actual-world and multiplayer QA on the pinned Horizon revision.
- [ ] Document every deliberate difference, migrate saves and review all notices.
- [ ] Call it a playable MVP only after real items process and visible machines operate.
- [ ] Call it a full port only after the major feature checklist and parity review are complete.
