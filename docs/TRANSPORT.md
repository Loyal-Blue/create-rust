# Transport: content 0.9.1 / native 0.10.1

Flat belts carry the game's real dropped-item stacks. Power a side shaft port with a motor, shaft or geared drive; the shaft axis is perpendicular to travel. Adjacent, aligned belts share rotation. Reversing the source reverses travel; loss of power, drive conflicts and overstress stop belts. Belt load is a prototype 1 stress unit per RPM per segment. Ordinary, unowned items on a belt have their despawn timer reset, including while the belt is stopped. Players can still pick stacks up.

Place individual belt blocks from the Create tab, or use a belt connector. Right-click one dry horizontal shaft, then a second parallel shaft at the same height, 1–16 blocks away and perpendicular to the shaft axis. The whole path must be loaded and empty between the shafts. A successful connection replaces the endpoints and fills the gap, costing one connector outside creative mode. A blocked or invalid plan changes no blocks and spends no connector. Sneak-click with the connector to cancel a pending selection; selections expire after two minutes.

Funnels attach to the inventory behind their facing direction. Their front opens toward the belt. Place facing away from the inventory, and use the wrench to rotate horizontally if needed.

| Control | Effect |
| --- | --- |
| Empty-hand right-click | Toggle insert/extract; default is insert |
| Sneak empty-hand right-click with a filter set | Clear the filter |
| Sneak empty-hand right-click without a filter | Cycle transfer amount 1 / 16 / 64 |
| Hold an item and right-click a brass funnel | Set an item-ID filter without consuming the held item |
| Sneak while holding a block | Allow ordinary adjacent placement |
| Redstone power | Pause the funnel |

Funnels attempt one transfer every eight ticks. Extracting requires a belt directly in front; it emits a stack above that belt and waits while its mouth remains occupied. Inserting takes nearby unowned item entities in front and places as many as fit, up to the configured amount. Full destinations leave remaining items in the world. Metadata on transferred stacks is preserved.

Machine and basin/depot insertion uses top-row inputs, and extraction uses middle-row outputs, preventing premature removal of raw ingredients. Chest, trapped chest, barrel, hopper, dispenser and dropper storage are also supported; a double chest currently exposes only the attached half. Furnaces, brewing stands and shulker boxes are deliberately excluded until their slot rules are integrated.

## Example: chest to millstone

These coordinates describe a straight run along +Z. All listed transport blocks are at Y=0.

| Position | Block | Setting |
| --- | --- | --- |
| (0, 0, -1) | Input chest | Put wheat here |
| (0, 0, 0) | Andesite funnel | Facing south; toggle to extract |
| (0, 0, 1) through (0, 0, 4) | Belts | Facing south |
| (-1, 0, 1) | Creative motor | Facing east, powering the belt's X shaft |
| (0, 0, 5) | Andesite funnel | Facing north; leave in insert mode |
| (0, 0, 6) | Millstone | Power separately through its bottom drive port |

Wheat travels as a visible world stack, enters the millstone's input row, and processes into its output row. Add an extracting funnel on another side of the millstone and another powered belt to carry the outputs onward. For a press/mixer line, attach inserting/extracting funnels to the depot/basin beneath the machine.

## Current limits

Belts are separate flat segments, not full Create multiblock structures. There are no slopes, corners, tunnels, covers, junction routing, moving players/mobs or automatic end ejection into inventories. Funnel models use static imported art; their mode/filter is reported in chat. Belts do not yet animate their surface. Presses and spouts can now process compatible loose belt stacks; fans process loose stacks and depots in their air stream. Mixers still work through basin inventories. See WORLD_PROCESSING.md for those arrangements. Brass filters match one item ID, not NBT/components or filter-item lists. Transport is paused in unloaded chunks and uses ordinary item-entity networking/storage.

The adapter has compiled and its core/host-stack tests pass; real gameplay, collision, rendering and multiplayer behavior still need in-game verification. This is not a complete Create port.
