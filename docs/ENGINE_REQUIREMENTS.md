# Required standard engine APIs (proposal, not implemented)

Verified engine revision: `47a3b39fa9e419ec03221fca5ef402d58e831f14`.
Sources: https://github.com/horizon-rust/horizon/blob/47a3b39fa9e419ec03221fca5ef402d58e831f14/docs/mods.md and `crates/horizon-mods/src/content.rs`.

API 0.2 copies vanilla templates. Block declarations reject unknown fields. `.hmod` code is rejected; the current engine's executable mod path is native. Adding a Cargo.toml or renaming a native file does not create a standard executable mod.

Needed upstream work:

| API | Contract needed by this project |
| --- | --- |
| State schema | Independent axis/facing and machine properties, stable identifiers and saved-state migrations |
| Shapes | Explicit collision/selection/occlusion boxes per state without template leakage |
| Placement | Authoritative pre-placement face, hit, view, sneak, adjacent drive ports; validate resulting shape before committing |
| Interaction | Server-side wrench actions, permissions and authoritative state updates |
| Simulation | Sandboxed fixed-tick callbacks with bounded world reads/writes and unloaded-chunk policy |
| Fluids | Flow vectors/falling-state queries around wheel perimeter |
| Rendering | Dynamic partial models and synchronized rotation phase, without rebuilding terrain every frame |
| Storage/network | Persist sources and machine data; synchronized topology/rotation to joining clients |
| Creative UI | Register Create category on inventory page index 1, page navigation and matching rendering, hover, focus and click handling |

Suggested rotation adapter: reconstruct loaded coaxial topology from authoritative states; sample actual flow vectors; call the prototype core once per tick; send RPM/jam/phase changes; interpolate partial-model rotation on clients; stop disconnected and unloaded networks; persist state by dimension and block position. Extend beyond the first-stage solver before adding gears/stress.

No upstream patch or runtime implementation of this proposal is included. This document identifies prerequisites instead of claiming unavailable APIs exist.
