# Create: Horizon — native development 0.3.0

The current work is the native Rust engine integration described in [NATIVE_DEVELOPMENT.md](NATIVE_DEVELOPMENT.md). It adds real machine slots, persisted progress/fluids, hopper access, server tick processing and network state payloads. Build it with `build-native.sh` against the pinned Horizon revision.

**The entire Create mod is not finished.** Contraptions, trains, belts, schematics, Ponder, full rendering and many other systems remain to be ported. [NATIVE_VALIDATION.md](NATIVE_VALIDATION.md) records what was actually verified.

The rest of this README documents the older 0.2 WASM/content alpha retained for reference. Its compiled binaries are separate from the new native source.

---

# Create: Horizon — 26.3 Alpha 0.2.0

A native Rust/WebAssembly **early port** for Horizon's Minecraft 26.3 engine.
This package contains two compiled `.hmod` files, reproducible source, tests,
original static models, and recipe data imported from your Create 6.0.10 JAR.

**This is not the full Create mod.** Kinetic networks work through server commands.
Recipe processing works in a Creative-only **virtual laboratory** powered by those
networks. Real item inventories, rotating block models, belts, fluid networks,
contraptions, trains and Ponder remain unfinished. The lab consumes and produces
virtual ID/count records; it neither takes nor gives real items.

## Install the compiled alpha

1. Extract the downloaded archive on Linux:

   ```sh
   tar -xzf create-horizon-26.3-alpha.tar.gz
   cd create-horizon-26.3
   ```

2. Locate **Horizon's game folder**, the folder containing its `options.txt`.
   Create `horizon-mods/` there and copy **both** files from `dist/`:

   ```sh
   # Set this to your actual Horizon game folder first.
   HORIZON_GAME_DIR="/absolute/path/to/your/horizon-game-folder"
   mkdir -p "$HORIZON_GAME_DIR/horizon-mods"
   cp dist/create.hmod dist/create_logic.hmod "$HORIZON_GAME_DIR/horizon-mods/"
   ```

   `create.hmod` registers the content (API 0.2, side `both`).
   `create_logic.hmod` runs the simulation (API 0.3, side `server`). Keep the pair
   at the same version. Replace the older prototype if it is installed: it used
   the same content namespace and an invalid combined manifest.

3. In Horizon, open **Options → Mods** and enable both entries, then open a fresh
   Creative test world. Close/reopen the world if enabling them while playing.
   A custom Horizon build needs `--features mods`; release builds include it.

4. For a dedicated/LAN server, install the content on the server and each Horizon
   client, and the logic on the host/server. The multiplayer sandbox test checks
   separate players' virtual inventories; real networked game sessions still need QA.

Target checked: Horizon commit
`87a6a7a161814ffffc846950aba4997f592df4fc`, API level **4** for the logic mod.
Update Horizon if it says the mod needs a newer API. Compatibility with another
Horizon revision is unverified. See [UPSTREAM.toml](UPSTREAM.toml).

## Try it

Use the commands in [examples/demo-commands.txt](examples/demo-commands.txt).
They make a motor → shaft → press at `0 100 0`. Use an empty spot in a disposable
Creative world. `/setblock` is a vanilla command and needs cheats/operator rights.
Blocks placed by `/setblock` are discovered with `/kinetics scan`.

```text
/create_slot my_factory_test
/setblock 0 100 0 create:creative_motor[axis=x]
/setblock 1 100 0 create:shaft[axis=x]
/setblock 2 100 0 create:mechanical_press[axis=x]
/kinetics scan 1 100 0 2
```

Wait for “Scan finished”, then:

```text
/kinetics info 2 100 0
/create_lab open pressing 2 100 0
/create_lab put minecraft:iron_ingot 2
```

After about two seconds, `/create_lab show` reports two **virtual** iron sheets.
`/create_save` flushes the selected slot immediately.

## Implemented

| Part | Behavior |
|---|---|
| Kinetic networks | Shafts, small cogs, large/small ratios, perpendicular large-cog bevels, signed RPM, stress, overstress, conflicting drivers, 256 RPM cap |
| Source choice | Uses the driving speed after all gear ratios; deterministic component solving |
| Sources | Creative motor, command-operated crank with ten-second expiry, simplified water-adjacency wheels |
| Transmission | Command-controlled clutch disconnection and gearshift reversal |
| Lifecycle | Place/break hooks, bounded scan queue, revalidation of tracked blocks every tick, unloaded blocks excluded from networks |
| Content | 25 blocks, 21 materials/parts, 32 crafting recipes, 26.3 item models and loot, original textures and shaped static models |
| Recipe interpreter | 338 supported recipes across 13 serializers; tags, repeated inputs, retained tools, fluids by ID/amount, heat and chance results |
| Laboratory | Nine station types attached to powered blocks, separate per-player stocks, bounded inventory/tanks, atomic crafting, blocked-output protection |
| Saves | Versioned data, validated reload, explicit world slots, virtual stock/progress/RNG persistence |

The server lab exposes milling, pressing, compacting, mixing, cutting, crushing,
deploying, splashing and haunting. Filling, emptying, sandpaper polishing and item
application are implemented/tested in the standalone interpreter, with no in-game
station integration yet. The drill is currently a kinetic stress consumer only.
Basins and depots are static decoration.

## Commands

| Command | Purpose |
|---|---|
| `/kinetics info [x y z]` | Speed, network state and stress; defaults to the block beneath you |
| `/kinetics scan [x y z] [radius]` | Queue discovery; default radius 5, maximum 12, 256 positions/tick |
| `/kinetics list` | Summary of the first eight loaded networks in your dimension |
| `/kinetics crank x y z` | Activate a crank for ten seconds |
| `/kinetics flip x y z` | Reverse a source or a gearshift's output half |
| `/kinetics toggle x y z` | Engage/disengage a clutch |
| `/create_lab open pressing x y z` | Attach your virtual lab to a compatible machine |
| `/create_lab put minecraft:iron_ingot 2` | Add virtual item inputs |
| `/create_lab fluid create:honey 250` | Add virtual fluid, measured in mB |
| `/create_lab heat heated` | Set the lab's heat (`none`, `heated`, `superheated`) |
| `/create_lab show` | Progress, status, virtual inputs and outputs |
| `/create_lab recipes pressing` | Search interpreted recipe IDs; at most six shown |
| `/create_lab clear` | Delete your virtual lab so another can be opened |
| `/create_slot factory_test` | Select a named persistence slot before building/scanning |
| `/create_save` | Save the selected slot now |

Setup, source controls, persistence and laboratory commands require Creative mode.
Diagnostics and discovery can be used in Survival. Server API 0.3 gives mods no
operator/permission API; Creative is the alpha's available authorization check.

Choose the world slot **before** placing/scanning blocks. Use a different slot for
each world and reselect it after reopening the world or reloading the mod. API 0.3
does not expose a world UUID or server shutdown callback. Nothing is automatically
restored from another world; selecting a slot replaces the in-memory registry and
labs. Selected slots autosave once per second when changed; use `/create_save`
before leaving to avoid losing the most recent second. Selecting an already
active slot leaves the current state intact. Leave free storage quota for new slots.

Limits: 512 remembered kinetic blocks, four pending scans, one scan per player,
16 labs total, nine virtual 64-item stack slots and 4,000 mB per input/output stock.
No claim is made that the maximum case meets the mod API's one-millisecond target.

## Known differences from Create

- Models are static and one block in size. Even large wheels do not occupy a 3×3
  area. Collision/occlusion inherit full-cube vanilla templates; visual holes are
  not passable. Axis placement uses basalt's three axis states and currently
  exposes both shaft ends for every registered kinetic block.
- Clutches disconnect the entire modeled node while off; separate rotating input
  halves are not simulated. Gearshifts report RPM at the negative-axis half.
  Controls are commands; redstone/block-use controls are not wired.
- Water wheels check adjacent water in the rotation plane. Flow direction, blade
  coverage, waterlogging and large-wheel multiblock validation are not ported.
- Processing time without an explicit JAR value defaults to 100 lab work units;
  work/tick is `abs(RPM)/32`, bounded to 0.03125–8. Mixing requires at least 32 RPM.
  Heat is a manual lab setting. Fans do not inspect an actual airflow medium.
- Recipes choose the first matching ID in sorted order. Resource tags use Create's
  shipped data plus an explicit small vanilla baseline. Missing dependency tags,
  unsupported ingredients and serializers are recorded in the import report.
- Item components, damage, durability and enchantments are not represented. Tool
  retention is supported, but retained axes do not lose durability. Fluid records
  are amounts in lab tanks, without pressure, pipes or fluid-world interaction.
- RNG is deterministic and persisted, but is not Java's RNG. Output space is
  reserved for the worst possible result before rolling; blocked retries cannot
  consume inputs or reroll. Input matching has a bounded search budget.
- There is no original Create differential oracle or proof of gameplay parity.

## Build from source

Needs a working Rust toolchain (1.95+), Git, Python 3.11+, a C linker, and Horizon's
matching authoring tool. The shipped build used **rustc 1.99.1**. On your Arch-based
Linux PC, use your existing Rust installation or install `rustup` through pacman;
there is no Java/NeoForge loader dependency in this Rust mod.

```sh
rustup target add wasm32-unknown-unknown
rustup component add rustfmt clippy
cargo install --git https://github.com/horizon-rust/horizon \
  --rev 87a6a7a161814ffffc846950aba4997f592df4fc \
  --locked cargo-horizon-mod
./build.sh
```

`build.sh` runs content validation, formatting, native tests, clippy with warnings
denied, both package builds, sandbox scripts and package checks. Results go in
`dist/`. It does not launch or install the game.

Generated assets/data are already included. Regenerate with:

```sh
python3 tools/gen_content.py
```

This generator intentionally rewrites `content/assets/` and `content/data/`. It
uses checked-in recipe references and needs no unpacked JAR or hardcoded home path.
To reimport a JAR, supply it explicitly, then regenerate and format:

```sh
python3 tools/import_recipes.py /path/to/create-1.21.1-6.0.10.jar
python3 tools/gen_content.py
cargo fmt --all
./build.sh
```

## Verification

See [VALIDATION.md](VALIDATION.md) and `dist/*-check.json` for actual results.
The native interpreter test crafts **every one of the 338 imported recipes** with
valid inputs. Sandbox scripts run the real `.wasm` through Horizon's Wasmtime
runtime, without a game window. **A running game's rendering, placement, creative
inventory, container interaction and network synchronization remain unverified.**
The development environment has no prepared, owned vanilla Minecraft resources,
so no visual/gameplay claim is inferred from sandbox success.

## Remaining work and licensing

[MISSING_PARTS.md](MISSING_PARTS.md) gives the concrete engine API blockers and
feature acceptance criteria. Full Create needs substantial additional engine and
mod development. The code/data follow MIT notices in [NOTICE.md](NOTICE.md).
Original textures/models are included; Create's All Rights Reserved assets,
Java classes and Mojang game files are excluded. This is an unofficial project.
