# Deployer applications: native 0.11.0

A powered deployer can apply materials or reusable tools to loose stacks on a flat belt or to inputs in a depot. Put held materials/tools in the deployer's top inventory row. Two-item `deploying` and `item_application` recipes are supported when their outputs exist in the installed registry. The first recipe ingredient belongs to the target; the second comes from the deployer. A complete deploying recipe stored in the deployer's own input row keeps its earlier priority.

For a downward deployer, leave one clear block between its body and the belt/depot: a deployer at Y=67 works on a surface block at Y=65. Power its X-axis shaft. Horizontal deployers can reach a supported surface two blocks ahead and one block below their body. Solid intervening blocks obstruct the reach. Redstone pauses deployment. Missing power or stress/conflict stops work without consuming materials.

Examples:

- Shafts on a belt/depot + held planks → small cogwheels.
- Stripped logs in a depot + held andesite alloy → andesite casings.
- Exposed copper in a depot + held axe → copper blocks, retaining the axe.

Each cycle consumes one operation's ingredient amounts. Component data on surviving input stacks and reusable tools remains unchanged. A changed target entity, position, quantity, held material or tool restarts the current operation. Depot products collect in the middle row; a blocked output row preserves both inputs and the recipe random state. The host reserves both inventories before committing. Loose inputs are held over the belt during work; products continue down the belt.

Outputs cannot receive another application from this deployer while they remain in its work footprint. Bounded per-controller entity/material guards also recognize stack-count changes caused by merging. They are pruned after items leave. Save/rejoin and entity merging still need running-game validation; this is not original Create's transported-item state system.

The Automation Lab has four dedicated application bays: depot cogwheels, belt cogwheels, a named reusable axe, and andesite casings. Nine isolated host-model tests exercise the production controller, commit and restore functions with real Horizon item stacks, including blocked outputs, pause/reach and repeated-output prevention. Engine physics and rendering are not part of that model.

Its headless checker completes their seeded batches and checks material counts, tool retention, drive speeds and support geometry.

Remaining gaps: general player-like use/attack, original durability rules, component-dependent recipes, fluid/heat applications, moving contraptions, arm animation and sequenced-assembly state. Reusable tools currently retain their damage rather than wearing out. This feature does not claim Java Create parity.
