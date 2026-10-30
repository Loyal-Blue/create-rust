# World processing: native 0.5.0

## Depots

Depots collect unowned dropped stacks resting on their top and store them in their real input slots. The first output stack, otherwise the first input stack, renders above the depot. Right-click with an item to insert it; empty-hand right-click collects output first, then input. If your inventory cannot accept it, the remainder stays on the depot. Sneak empty-hand right-click opens the retained 27-slot prototype inventory. Sneaking while holding a block passes through to normal placement.

Funnels can insert into depot inputs and extract processed outputs. The prototype can queue several stacks; the preview shows only the first eligible stack rather than drawing every queued item.

## Presses

Place a press one or two blocks above a belt or depot, with its horizontal shaft connected to power. Two-block spacing leaves clearance for its moving head. With no inputs in the press's own inventory, compatible belt stacks are centered and held during processing, then released. Depot contents take priority when a depot below has inputs. Pressing converts up to 64 compatible items in one work cycle, reserving all outputs first. At 16 RPM, a normal 100-work recipe takes about 2.5 seconds at 20 ticks per second.

New compatible items joining a stack do not starve work. A different entity, material, metadata change or reduced world-stack count resets its job. Persistent machine data stores binding hashes/counts, progress and RNG, not another inventory of the world items. If an entity gets a new ID across a reload its unfinished work may restart; the item remains the authoritative stored entity.

The press's own input row still takes priority over external targets. Over a basin, the existing compacting/pressing fallback remains available. Its shaft and head now render as separate moving parts; collision and selection remain the static authored shapes.

## Spouts and drains

Put a spout one or two blocks above a belt or depot. Feed it with buckets or a pipe network. Compatible belt stacks wait while filling and release when finished. Each filling operation reserves its item and fluid before committing; filling is one item per cycle. With no compatible fluid available, a belt item is allowed to pass rather than reserve an unfulfillable job.

Generic filling supports water, lava, milk, honey and chocolate buckets, plus water bottles (250 mB). Water-bottle outputs include the water potion contents component. Existing honey, chocolate, tea and blaze-cake recipes remain supported. Item drains can empty those five filled bucket types through their input/output inventory, returning an empty bucket and storing 1,000 mB of fluid. Loose-item draining and arbitrary potion/third-party container conversion remain absent.

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
