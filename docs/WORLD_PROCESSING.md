# World processing: native 0.7.0

## Depots

Depots collect unowned dropped stacks resting on their top and store them in their real input slots. The first output stack, otherwise the first input stack, renders above the depot. Right-click with an item to insert it; empty-hand right-click collects output first, then input. If your inventory cannot accept it, the remainder stays on the depot. Sneak empty-hand right-click opens the retained 27-slot prototype inventory. Sneaking while holding a block passes through to normal placement.

Funnels can insert into depot inputs and extract processed outputs. The prototype can queue several stacks; the preview shows only the first eligible stack rather than drawing every queued item.

## Presses

Place a press one or two blocks above a belt or depot, with its horizontal shaft connected to power. Two-block spacing leaves clearance for its moving head. With no inputs in the press's own inventory, compatible belt stacks are centered and held during processing, then released. Depot contents take priority when a depot below has inputs. Pressing converts up to 64 compatible items in one work cycle, reserving all outputs first. At 16 RPM, a normal 100-work recipe takes about 2.5 seconds at 20 ticks per second.

New compatible items joining a stack do not starve work. A different entity, material, metadata change or reduced world-stack count resets its job. Persistent machine data stores binding hashes/counts, progress and RNG, not another inventory of the world items. If an entity gets a new ID across a reload its unfinished work may restart; the item remains the authoritative stored entity.

The press's own input row still takes priority over external targets. Over a basin, the existing compacting/pressing fallback remains available. Its shaft and head now render as separate moving parts; collision and selection remain the static authored shapes.

## Spouts and drains

Put a spout one or two blocks above a belt or depot. Feed it with buckets or a pipe network. Compatible belt stacks wait while filling and release when finished. Each filling operation reserves its item and fluid before committing; filling is one item per cycle. With no compatible fluid available, a belt item is allowed to pass rather than reserve an unfulfillable job.

Generic filling supports water, lava, milk, honey and chocolate buckets, plus water bottles (250 mB). Water-bottle outputs include the water potion contents component. Existing honey, chocolate, tea and blaze-cake recipes remain supported. Item drains can empty those five filled bucket types through their input/output inventory, returning an empty bucket and storing 1,000 mB of fluid. Item drains also process compatible loose containers resting on their top surface; stored input rows keep priority. Fluid output backpressure stops completion without consuming the live container. Arbitrary potion/third-party container conversion remains absent.

## Fans

Connect the fan's rear drive. It moves item entities through up to eight blocks in front, stops its reach at unloaded chunks or blocks with full collision, and reverses the force when RPM reverses. Vertical airflow compensates item gravity. Players and mobs are not transported yet.

| Medium immediately in front | Processing |
| --- | --- |
| Water | Splashing/washing |
| Lava | Blasting |
| Fire or lit campfire | Smoking |
| Soul fire or lit soul campfire | Haunting |

Airflow also works without a processing medium. Compatible loose stacks in the stream can process in bulk. A compatible depot in the stream is processed through its real slots. A fan handles one active loose stack or depot at a time; this is not full parallel exposure tracking. It still supports its own inventory, which takes priority. Fluids, fire and ordinary item pickup remain normal engine mechanics.

## Visuals and limits

Press shafts/heads, mixer gears/poles/whisks, and depot stacks use separate client rendering. Work phases extrapolate briefly from server snapshots. Content-only previews retain the complete static assemblies; only the native resource cache hides the moving pieces. These are prototype animations, without moving collision, entity damage, sound/particle parity or running-game visual verification.

The expanded build has compiled and passed automated checks. It still needs in-game collision, save/rejoin, rendering and multiplayer verification. There are no moving contraptions, trains, sloped belts or complete Create parity.

## Crushing wheels

Place two horizontal-axis wheels at the same height with one block of air between them, perpendicular to their shaft axis. Drive both toward the gap: X-axis shafts require positive RPM on the lower-Z wheel and negative RPM on the higher-Z wheel; Z-axis shafts require negative RPM on the lower-X wheel and positive RPM on the higher-X wheel. Their signs refer to this port’s shaft convention. Opposite outward motion, vertical shafts and unpowered pairs do not crush world items.

Drop a compatible stack into the gap. The pair holds one stack during work, processes it in bulk at the slower wheel’s speed and releases outputs below. Both wheels must have empty input rows to use the world route; existing wheel inventories keep priority. This uses an authored gap check, not Create’s full controller/collision structure.

## Saws and wood processing

An upward-facing powered saw cuts one item per cycle from a loose stack resting on its top. Its own input row takes priority. The remaining stack keeps its components, and new outputs are ordinary dropped item entities. Downward/horizontal saws do not use the top-item path. Outputs are not automatically ejected into belts yet.

The authored wood recipes cover nine overworld woods plus crimson and warped wood: log/stem stripping, wood/hyphae stripping, six planks from stripped logs/wood, two slabs or one stair from a plank. Hold a desired output (for example oak stairs) and right-click a saw to set its output filter without consuming the held item. Sneak empty-hand right-click clears the selector. Empty-hand normal right-click still opens the inventory; sneaking with a block allows placement. With no selector, the first compatible recipe runs. A selector cannot skip required stripping steps. It applies to inventory and loose-item cutting. Selector clicks currently run on the local host; remote-client selector controls remain unimplemented.

A horizontal powered saw with empty input rows attempts to fell an unstripped overworld log directly in front. After a work interval it plans a same-species connected trunk/branch set at or above the cut, with matching nearby leaves. It leaves roots below the cut intact. The complete plan must stay within six blocks horizontally, 32 blocks vertically, 128 logs and 4,096 examined cells. Any unloaded neighbor or oversized plan aborts before breaking logs. Ordinary engine block breaking supplies the drops and leaf updates. Bare log pillars and stripped logs are refused; player-built log structures attached to leaves can still qualify. This prototype does not implement nether fungus trees, original leaf-distance detection, entity damage or contraption saws.

## Chutes

Vertical chutes catch falling/resting unowned item stacks above their own opening, respecting their real inventory capacity. Stacks held by an active processor are excluded until that job releases them. Existing top/bottom inventory transfers remain available. With no inventory below, they release one item at a time into air, water or onto a flat belt. A nearby discharged stack pauses further release until it falls/moves away; a blocked or unloaded outlet retains the contents. Component data survives capture and discharge. There is no smart-chute filtering or item-inside-chute animation yet.
