# Validation for content 0.4.0 / native 0.2.0

- Ten engine-independent Rust core tests pass.
- Seven native-support fixtures pass: body-shape replacement, creative item/search completeness, safe overlay replacement, page-2 visibility, old motor axis/waterlogging migration, upright-machine migration, and modern-facing preservation.
- horizon-mixins stages nine HEAD hooks and two support modules against Horizon 47a3b39fa9e419ec03221fca5ef402d58e831f14.
- Patched `horizon-client` game binary passes `cargo check --locked -p horizon-client --bin horizon-client --no-default-features`.
- Content and native archives pass Horizon package/model validation.
- Root workspace passes formatting and clippy with warnings denied.

No game was launched: player-owned prepared resources are not available in this environment. Compilation and these fixture tests do not establish gameplay, rendering or exact Create parity. Wheel/shaft animation is a client-side prototype; there is no server-authoritative processing/stress simulation yet. The placement hook corrects a successfully placed state; it does not yet replace every pre-placement collision/clearance check. No original artwork was uploaded publicly.
