# Status

The active path is now the experimental native runtime, plus a content pack. Standard `.hmod` files still cannot run the machinery code.

| Feature | Native implementation | Verification limit |
| --- | --- | --- |
| Model-based hitboxes | Existing box unions retained | Approximate static geometry |
| Directional placement | Facing states, view/face/neighbor policy, upright machines | Engine compiles; no in-game parity test |
| Saved states | Axis/waterlogged → facing migration for changed machines | Fixture tests |
| Wrench | Right-click rotates; sneak reverses | Engine compilation |
| Water wheel → shaft | Flow torque, coaxial topology, dynamic client meshes | Core tests and engine compilation; visual only |
| Creative page 2 | Page navigation, hidden off-page categories, shared input positions | Visibility tests and engine compilation |
| Processing/stress/gears | Not implemented in current runtime | Historical experiments are not enabled |

The original input JAR and imported artwork remain local assets. Public source builds require a local asset input; this is not yet an independently redistributable playable release.
