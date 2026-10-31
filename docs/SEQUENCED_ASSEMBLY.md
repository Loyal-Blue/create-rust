# Precision mechanism assembly: native 0.12.0

The first sequenced recipe is implemented. Apply a small cogwheel, a large cogwheel and an iron nugget to a golden sheet, in that order, repeated five times. Each of the 15 operations consumes one held material: one attempt needs one sheet and five of each material.

Use powered deployers above belts or depots. Bay 30 contains a complete straight line of 15 downward-facing deployers, preloaded for 16 attempts. Their X-axis shafts share one drive; the belt has another. Release both drive clutches, then the feed control. The unfiltered outlet chest collects successes and byproducts.

Intermediate progress travels with the item in `minecraft:custom_data`, under this port's `create:assembly` key. Names, lore and unrelated custom data are preserved through transfers and save codecs. The assembly key is removed at completion. Blank creative-menu intermediates have no progress and cannot skip the sequence.

Wrong materials, missing power and invalid/future progress cannot consume ingredients. Changed targets or held materials restart only the current operation; completed item stages remain recorded. Depot space is reserved before either input is consumed. A blocked final output preserves both inputs and the saved RNG, so freeing space resumes the same rolled result.

Result weights total 150: precision mechanism 120; golden sheet 8; andesite alloy 8; small cogwheel 5; gold nugget 3; shaft 2; crushed raw gold 2; iron ingot 1; clock 1. Each attempt yields one final item, which may be a byproduct. This port uses its saved prototype RNG, not Java's random sequence. Intermediate items use the original static sprite and a stack limit of one.

Recipe facts and the two local item models/textures were checked against the official Maven build `create-1.21.1-6.0.10-258.jar`, SHA-256 `2e9b36ba86a5b99a1876748d642b3af9c3344221f2b9bc3c2cc0a79d4e6fedab`. Original art remains in the private local build; source contains declarations, identifiers and import tooling. Input: https://maven.createmod.net/com/simibubi/create/create-1.21.1/6.0.10-258/

Checks cover ordered stages, weight intervals, malformed progress, actual item-NBT roundtrips, bulk splitting, foreign components, missing registry support, no power and blocked-final-output recovery. An isolated host model runs the production controller through a complete 15-deployer belt line. The map checker completes its 16 attempts using production item/application/save codecs.

Other sequences, press/spout/cutting steps, arbitrary data-pack assembly loading, original loop routing and progress-dependent item animation remain absent. Real physics, save/rejoin, rendering and multiplayer still need in-game verification. This is one recipe path in an unfinished port.
