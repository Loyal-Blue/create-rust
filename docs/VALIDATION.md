# Validation: content 0.5.1 / native 0.3.1

Checked on 2026-10-30 against Horizon `47a3b39fa9e419ec03221fca5ef402d58e831f14` and horizon-mixins `cb1ad23022392d88f006cf94cbf6552a73f5df12`.

- Root Rust workspace: 73 tests passed (10 placement, 33 transmission, 19 machine transactions/state, 11 recipe simulation).
- Ten native fixtures passed: three additional real Horizon ItemStack bridge tests cover parsed motor/press drive ports, processing a named iron stack, unpowered preservation, and retention of a damaged/named deployer tool. The seven earlier fixtures cover body shapes, creative/search completeness, safe symlink overlay replacement, page visibility, saved-state migration and facing preservation.
- Geared press test connects a motor through a 2:1 cog pair, processes live-style slots and reloads machine state between ticks. Other tests cover stress stalls, source conflicts, configured RPM validation, output backpressure, stale inventory commits, component preservation, fluid transfer, burner timers, slot automation and bounded deterministic ore placement.
- Workspace formatting and clippy with warnings denied pass.
- horizon-mixins stages 18 HEAD hooks and two modules. Patched `horizon-client` passes `cargo check --locked -p horizon-client --bin horizon-client --no-default-features`.
- Horizon package/model validation: 36 registered blocks, 36 standalone items, 36 blockstate definitions, 72 client item definitions and 142 model JSON files; all model references resolve and bake (26,902 quads). Vanilla parents use minimal structural stubs for this check, not player resources.

No game was launched: player-owned prepared game resources are unavailable here. Compilation and fixtures do not prove gameplay, rendering, save/rejoin behavior or multiplayer correctness. This build implements server-side processing and state synchronization, but their integration still needs running-game verification. Placement corrects the successfully placed state; not every pre-placement clearance/collision rule is replaced. See FUNCTIONAL_SCOPE.md for approximations and missing systems.

Original artwork remains a local build input and was not uploaded publicly.

Native 0.3.1 regression checks cover basin pressing fallback without resetting progress, a press finishing at water-wheel speed, component-bearing ingredients, input-row selection, status reasons, rotation packet continuity, speed changes and bounded prediction after snapshots stop. Host bridge fixtures extract the production portable/restore functions rather than copy their logic. No in-game latency measurement or complete running-game reproduction was possible; the reported gameplay symptoms are addressed by these code paths and tests, pending player verification.
