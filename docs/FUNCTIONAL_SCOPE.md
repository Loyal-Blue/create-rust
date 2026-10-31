# Functional scope: native 0.10.0

These are implemented code paths, not claims of in-game validation or exact Java parity.

| Blocks/system | Implemented behavior | Remaining gaps |
| --- | --- | --- |
| Creative motor, hand crank | Configurable 0–256 RPM; reversal; timed crank | No original speed GUI; crank animation |
| Small/large water wheel | Flow-vector torque, 8/4 RPM, synchronized rotating meshes | Full structure/clearance checks and original water rules |
| Shaft, small/large cogs | Ports, ratios, speed inversion, animated meshes | Exact tooth geometry/phase and all original connection cases |
| Gearbox, encased chain drive | In-plane drive transmission | Original chain range rules and casing interactions |
| Clutch, gearshift | Manual and redstone disengage/reversal | Original GUIs/animated internals |
| All drive networks | Stress capacity/load, source conflicts, stalled RPM | Exact original stress configuration; unloaded network parity |
| Mechanical press | Bulk depot/belt pressing; compacting over basin; moving head/shaft | Moving collision, sounds/particles and exact original timing |
| Mechanical mixer | Mixing at supported RPM/heat in basin below; moving whisk/pole/gear | Exact whisk stroke/collision, original speed/timing rules |
| Millstone, crushing wheel | Container milling/crushing; inward horizontal wheel pairs crush loose stacks in bulk | Full original crushing-controller structure and collision |
| Mechanical crafter | Powered complete rectangle up to 9x9; Horizon crafting/remainders; reserved multi-inventory commits; 5x5 crushing-wheel recipe | Original arrow routing, input covers, lid/cog/transport animations, arbitrary nonrectangular networks and world-dependent special recipes |
| Mechanical saw | Container/top-facing loose-stack cutting; output selector; bounded horizontal tree felling | Entity damage, moving contraptions and complete original tree detection |
| Mechanical drill | Breaks a breakable non-Create block ahead with ordinary drops | Original hardness timing, machine mining, moving contraption use |
| Encased fan | Inventory, depot and loose-stack processing; bounded item airflow through appropriate media | Parallel exposure tracking, players/mobs and original fan visuals |
| Deployer | Container deploying recipes; retained catalysts preserve damage/components; powered placement of plain supported building cubes, with redstone pause | Full player-like use/attack, directional/device placement, crops, contraptions and arm animation |
| Basin, depot | Persistent 27-slot storage; visible depot stack, dropped-item capture and direct depot interactions | Exact original depot capacity, basin geometry/fluid display |
| Blaze burner | Coal/charcoal/blaze-rod heat; blaze-cake superheat | Capturing blazes, original heat transitions and visuals |
| Fluid tank | Shared complete 1x1/2x2/3x3 square tanks up to 32 layers; 8,000 mB per block; any-member bucket/pump access; legacy contents retained | Original tank model states, fluid rendering, connectivity rules and remote bucket controls |
| Fluid pipe, mechanical pump | Signed RPM routes across loaded branches, 16 steps/256 nodes, one shared budget; renewable-water intakes | Original arm geometry, pressure, valves, finite-fluid extraction, pump-series propagation |
| Spout, item drain | Spout inventory/depot/belt filling, proper water bottles, five generic bucket types; inventory/top-surface draining | Arbitrary potion conversion and original fluid rendering |
| Flat belt | Real dropped-item movement; straight shaft coupling; reversal/stalls; connector placement | Slopes, junctions, covers, entity transport and belt-surface animation |
| Andesite/brass funnel | Import/export, redstone pause, amounts, brass item-ID filter, machine slot rules | Smart filter lists, NBT matching, full original model states and side rules |
| Chute | Vertical inventory transfers, falling-stack capture and one-item discharge into air/water/flat belts; component-safe snapshots | Original smart chutes, filters and entity rendering through chute |
| Hoppers | Machine top-row insertion/middle-row extraction | Original side-specific automation rules |
| Zinc/deepslate zinc ores | Fresh overworld generation, raw zinc drops, smelt/blast zinc | Exact distribution, fortune/silk rules; no retro-generation |
| Casings, alloy/brass/zinc blocks | Placeable material blocks and recipes | Interactive encasing of drives |
| Standalone materials/foods/wrench | Registered art, recipes, wrench rotation; explicit food values, tea Haste and XP-nugget use | Exact original effects/food balance; remote XP-nugget use |
| Creative inventory | 77 entries on page 2 with search | Running-game layout/input verification |
| Persistence/network | Chunk metadata for machines; ordinary block-entity item storage; server snapshots | Real save/rejoin/multiplayer verification |

Recipe simulation includes 415 active entries (338 imported recipes plus 11 generic container operations and 66 authored wood-cutting recipes). The adapter only considers recipes whose item outputs exist in the host registry, so uninstalled decorative outputs cannot consume inputs or block supported recipes. The tag matcher, work durations, chances and machine inventories are prototype rules rather than exact Create behavior. Content crafting/smelting uses the engine data pack path.

Sloped/branched belts, tunnels, sequenced assembly world machines, schematics, moving contraptions, bearings, pistons, elevators, trains and most of Create's decorative/content catalogue are not implemented. They require substantial further work.
