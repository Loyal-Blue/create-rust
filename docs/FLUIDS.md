# Fluid routing: native 0.9.0

A pump routes fluid between ports on the two sides of its axis. Positive/negative RPM choose opposite inlet/outlet sides. Pipes can branch and turn through face-adjacent pipe blocks; tanks, basins, spouts, item drains and mechanical mixers are endpoints. Endpoints do not act as additional pipes, and pumps do not traverse other pumps.

Each scan is limited to 16 steps from a pump side and 256 visited nodes. Unloaded pipes are absent from the graph. Oversized components and networks reconnecting inlet to outlet are blocked without moving fluid. The pump spends one total budget of `abs(RPM) × 4` mB per tick, capped at 1,024 mB/t, sharing it among available outlets and rotating endpoint priority over time. A full or incompatible outlet does not consume its share of fluid. Each pump reads the latest state, so multiple pumps cannot spend the same stored fluid twice.

New tank fills hold one fluid. Complete square 1x1, 2x2 and 3x3 footprints up to 32 layers share 8,000 mB per member block, including input/output storage combined. Buckets and pump pipes can access the whole shared volume through any member. Tank members are port aliases, so a pump cannot circulate fluid between two sides of the same shared tank. Older larger or mixed contents remain saved and can be drained; new incompatible fills are refused. Basins and machine stores retain the prototype's separate multi-fluid input/output buffers. Bucket extraction can combine the same fluid from both buffers atomically. Refused bucket interactions do not dump the bucket into the world.

## Renewable water

A water source touching a pump or a connected pipe is an intake when it has at least two cardinal water-source neighbors and solid support or another water source below. The relevant cells must be loaded. The route is still limited by the same pump budget and graph bounds; only this verified renewable intake supplies new water. It is not saved as a phantom tank and cannot receive reverse flow or other fluids. Full/incompatible destinations draw nothing. Single finite sources, flowing water, lava and waterlogged blocks are not extracted.

Pipe visuals remain simple static geometry: branching flow does not yet create original connected arm/casing models. There is no pressure simulation, finite-fluid extraction, hose pulley, valve, pump-series propagation, original tank geometry/visibility rules or exact original fluid behavior. These code paths compile and pass conservation tests, but have not been exercised in a running game.

## Shared storage and reshaping

Fluid is persisted only in member blocks, never a second controller inventory. Aggregate views live only during a transaction and are distributed bottom-up before saving. Individual records stay within their normal capacity and retain their unrelated state. Adding/removing/reloading members rebuilds the view from those records; fluid in a removed block is not recovered automatically. Irregular/oversized connected shapes remain independent stores. Existing mixed or overfull legacy stores are excluded from merging and remain drainable. A group expansion must succeed for every endpoint before pump writes are committed. No original connected tank models or fluid-surface rendering are implemented yet.

Sneak empty-hand right-click reports a valid shared tank’s volume and capacity. Bucket controls currently run on the local host; remote controls and actual save/rejoin/multiplayer behavior remain unverified.
