# Native validation — 2026-10-30

Target: Horizon `87a6a7a161814ffffc846950aba4997f592df4fc`, Minecraft 26.3, Rust 1.99.1 on Linux x86_64.

| Check | Result |
|---|---|
| Kinetic core unit tests | 30 passed |
| Recipe simulation unit tests | 11 passed |
| Native runtime unit tests | 10 passed |
| Native save/load + network ID marker unit test | 1 passed |
| Core workspace Clippy, all targets, warnings denied | Passed |
| Native Horizon client `cargo check`, no default features, `create` enabled | Passed |
| Native Horizon client Clippy, warnings denied | Passed |
| Horizon client compile with Create disabled | Passed |
| Native Linux client linked build, debug profile | Passed |
| Horizon package/content parser | Passed: 34 blocks, 25 separate items, 41 recipes |
| Content reference validation | 59 declared IDs, 351 JSON files, no dangling local references |
| Patch against clean pinned checkout | Applied successfully, repeat application accepted, `git diff --check` clean |

The runtime tests cover real input/output count changes, cancellation after a stale inventory edit, preserving opaque item components, full-output rejection without input or RNG loss, tank capacity and serialization, malformed state rejection, component-preserving transfer, full-destination rejection, fluid conservation and pump budget, and continuous saved rotation phase.

The engine save test serializes actual native block entity slots and data through Horizon's NBT encoder/decoder. It also decodes the chest protocol carrier with the native ID marker. This is a serialization test, not a live multiplayer session.

No prepared player-owned Minecraft resources were available. A world was not launched, and rendering, survival progression, integrated-server gameplay and network sessions remain unverified. No Java parity oracle was run. Existing `VALIDATION.md` and legacy `dist/*-check.json` files describe the earlier 0.2 laboratory, not this native code.

New content package: 34 blocks, 25 separate items and 41 crafting recipes. The processing catalog retains 338 supported recipes, but that count is not a claim that every recipe can run with the currently registered content. Unknown outputs are refused by native processing.
