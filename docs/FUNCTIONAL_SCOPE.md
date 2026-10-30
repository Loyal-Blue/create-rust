# Functional scope: native 0.3.1

These are implemented code paths, not claims of in-game validation or exact Java parity.

| Blocks/system | Implemented behavior | Remaining gaps |
| --- | --- | --- |
| Creative motor, hand crank | Configurable 0–256 RPM; reversal; timed crank | No original speed GUI; crank animation |
| Small/large water wheel | Flow-vector torque, 8/4 RPM, synchronized rotating meshes | Full structure/clearance checks and original water rules |
| Shaft, small/large cogs | Ports, ratios, speed inversion, animated meshes | Exact tooth geometry/phase and all original connection cases |
| Gearbox, encased chain drive | In-plane drive transmission | Original chain range rules and casing interactions |
| Clutch, gearshift | Manual and redstone disengage/reversal | Original GUIs/animated internals |
| All drive networks | Stress capacity/load, source conflicts, stalled RPM | Exact original stress configuration; unloaded network parity |
| Mechanical press | Pressing; compacting over basin; depot/basin 1–2 below | Moving press head and physical transported items |
| Mechanical mixer | Mixing at supported RPM/heat in basin below | Moving whisk and original minimum-speed rules |
| Millstone, crushing wheel | Container milling/crushing recipes | Opposed wheel pair and world-item crushing |
| Mechanical saw | Container cutting recipes | Tree felling and world saw damage |
| Mechanical drill | Breaks a breakable non-Create block ahead with ordinary drops | Original hardness timing, machine mining, moving contraption use |
| Encased fan | Own-inventory splashing/blasting/smoking/haunting selected by adjacent water/lava/fire/soul fire | Air streams, entity pushing and dropped-item processing |
| Deployer | Container deploying recipes; retained catalysts preserve damage/components | Player-like block/entity interactions and world arm animation |
| Basin, depot | Persistent 27-slot storage; external processing when machine input row is empty | Original single-item rendering, basin geometry/fluid display |
| Blaze burner | Coal/charcoal/blaze-rod heat; blaze-cake superheat | Capturing blazes, original heat transitions and visuals |
| Fluid tank | Persistent fluid input/output stores, 8,000 mB capacity each | Multiblock merging, original visuals and capacity rules |
| Fluid pipe, mechanical pump | Signed RPM moves fluids through straight runs up to 16 pipes | Branches, valves, water-source extraction, original pump drive topology |
| Spout, item drain | Filling/emptying recipes without kinetic power; spout can target depot below | World item transport and original fluid rendering |
| Chute | One-item vertical transfers per tick; component-safe snapshots | Original falling-item behavior, smart chutes and filters |
| Hoppers | Machine top-row insertion/middle-row extraction | Original side-specific automation rules |
| Zinc/deepslate zinc ores | Fresh overworld generation, raw zinc drops, smelt/blast zinc | Exact distribution, fortune/silk rules; no retro-generation |
| Casings, alloy/brass/zinc blocks | Placeable material blocks and recipes | Interactive encasing of drives |
| Standalone materials/foods/wrench | Registered art, recipes, wrench rotation | Tea effects, XP-nugget use, full original food effects |
| Creative inventory | 72 entries on page 2 with search | Running-game layout/input verification |
| Persistence/network | Chunk metadata for machines; ordinary block-entity item storage; server snapshots | Real save/rejoin/multiplayer verification |

Recipe simulation includes 338 entries. The adapter only considers recipes whose item outputs exist in the host registry, so uninstalled decorative outputs cannot consume inputs or block supported recipes. The tag matcher, work durations, chances and machine inventories are prototype rules rather than exact Create behavior. Content crafting/smelting uses the engine data pack path.

Belts, funnels, tunnels, mechanical crafters, sequenced assembly world machines, schematics, moving contraptions, bearings, pistons, elevators, trains and most of Create's decorative/content catalogue are not implemented. They require substantial further work.
