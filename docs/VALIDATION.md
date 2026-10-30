# Validation: content 0.5.0 / native 0.3.0

Checked on 2026-10-30 against Horizon `47a3b39fa9e419ec03221fca5ef402d58e831f14` and horizon-mixins `cb1ad23022392d88f006cf94cbf6552a73f5df12`.

- Root Rust workspace: 69 tests passed (10 placement, 33 transmission, 15 machine transactions/state, 11 recipe simulation).
- Seven native fixtures passed: body shapes, creative/search completeness, safe symlink overlay replacement, page visibility, saved-state migration and facing preservation.
- Geared press test connects a motor through a 2:1 cog pair, processes live-style slots and reloads machine state between ticks. Other tests cover stress stalls, source conflicts, configured RPM validation, output backpressure, stale inventory commits, component preservation, fluid transfer, burner timers, slot automation and bounded deterministic ore placement.
- Workspace formatting and clippy with warnings denied pass.
- horizon-mixins stages 17 HEAD hooks and two modules. Patched `horizon-client` passes `cargo check --locked -p horizon-client --bin horizon-client --no-default-features`.
- Horizon package/model validation: 36 registered blocks, 36 standalone items, 36 blockstate definitions, 72 client item definitions and 142 model JSON files; all model references resolve and bake (26,902 quads). Vanilla parents use minimal structural stubs for this check, not player resources.

No game was launched: player-owned prepared game resources are unavailable here. Compilation and fixtures do not prove gameplay, rendering, save/rejoin behavior or multiplayer correctness. This build implements server-side processing and state synchronization, but their integration still needs running-game verification. Placement corrects the successfully placed state; not every pre-placement clearance/collision rule is replaced. See FUNCTIONAL_SCOPE.md for approximations and missing systems.

Original artwork remains a local build input and was not uploaded publicly.
