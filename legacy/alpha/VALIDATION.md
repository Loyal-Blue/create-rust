# Validation — 29 October 2026

Target: Minecraft **26.3**; Horizon
`87a6a7a161814ffffc846950aba4997f592df4fc`.
Compiler: **rustc 1.99.1 (58cf8f9c3 2026-10-13)**, Linux x86-64.

| Check actually run | Result |
|---|---|
| Original prototype baseline | 18 kinetic tests passed before changes |
| `cargo test --workspace --locked` | 28 kinetic tests + 11 recipe/stock tests passed |
| SDK documentation tests | 2 passed; 1 upstream example intentionally ignored |
| All imported lab recipes | Each of 338 recipes completed with valid inputs in a native test |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `python3 tools/validate_content.py` | 46 declared IDs and 284 JSON files; local model/texture/item/tag references resolve |
| Real SDK source/WIT | Compared byte-for-byte with the pinned upstream sources |
| `cargo horizon-mod build` in `content/` | Compiled content package produced |
| `cargo horizon-mod build` in `server/` | Real wasm32 component built and packed against Horizon's SDK |
| `cargo horizon-mod check dist/create.hmod --json` | `ok: true`, API 0.2, content-only, side `both` |
| `cargo horizon-mod check dist/create_logic.hmod --json` | `ok: true`, API 0.3, server component, API level 4 |
| `cargo horizon-mod test --hmod ../target/horizon-mod/create_logic.hmod` | 5 sandbox scripts, 46 steps, all passed |
| Extra `tools/runtime_probe.rs` using Horizon's runtime | Loaded → unavailable → reloaded chunk view passed; remembered networks resumed without rescanning |

The five sandbox scripts cover:

- `kinetics.toml`: discovery, signed RPM/stress, source reversal, crank expiry,
  removal of a bridge and stale block cleanup.
- `transmission.toml`: gearshift output sign, clutch disconnection and reconnection.
- `water.toml`: water activation and loss of water.
- `laboratory.toml`: two actual JAR pressing recipes, manual save, reload, slot
  restore, lost power, Creative authorization and an incompatible replacement block.
- `multiplayer.toml`: independent virtual item stocks for two players sharing
  the same powered machine; refused overflow leaves stock unchanged.

Native regressions also include all bevel-axis/sign combinations, exact stress
thresholds, source selection through multiple ratios, overspeed, world-coordinate
overflow, explicit shaft ports and a 362-node reduction train whose relative
ratios exceed `f32`'s range. Recipe tests cover no power, heat/speed requirements,
blocked outputs without input loss or reroll, retained catalysts, repeated inputs,
fluid accounting and validated save/reload of progress and random state.

`dist/content-check.json` and `dist/logic-check.json` are the authoring tool's
actual machine-readable reports. Its generic trial invokes commands without
arguments and in Survival; expected required-argument/Creative errors therefore
appear in the logic report's `output`. These are ordinary command replies.
`ok: true` reports successful validation and an untrapped sandbox trial.

## Verification boundary

There was **no running Horizon game** in this environment and no prepared owned
vanilla resource root. Sandbox `WorldView` tests supply block strings rather than
the game's full registry, physics, saves, renderer or network. JSON/reference
checks do not prove all 25 blocks visually render or interact correctly in-game.

Still unverified: real registry loading and content handshakes, creative inventory,
crafting/drop behavior in the game, full-cube collisions, visual models, real
single-player/LAN/dedicated sessions and performance at maximum limits.
No Java-original differential oracle, Java timing/RNG parity, survival machine
inventory integration or full Create feature parity is claimed.
