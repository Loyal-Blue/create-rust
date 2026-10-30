# Fluid routing: native 0.6.0

A pump routes fluid between ports on the two sides of its axis. Positive/negative RPM choose opposite inlet/outlet sides. Pipes can branch and turn through face-adjacent pipe blocks; tanks, basins, spouts, item drains and mechanical mixers are endpoints. Endpoints do not act as additional pipes, and pumps do not traverse other pumps.

Each scan is limited to 16 steps from a pump side and 256 visited nodes. Unloaded pipes are absent from the graph. Oversized components and networks reconnecting inlet to outlet are blocked without moving fluid. The pump spends one total budget of `abs(RPM) × 4` mB per tick, capped at 1,024 mB/t, sharing it among available outlets and rotating endpoint priority over time. A full or incompatible outlet does not consume its share of fluid. Each pump reads the latest state, so multiple pumps cannot spend the same stored fluid twice.

New tank fills hold one fluid and enforce 8,000 mB across input/output storage combined. Older larger or mixed contents remain saved and can be drained; new incompatible fills are refused. Basins and machine stores retain the prototype's separate multi-fluid input/output buffers. Bucket extraction can combine the same fluid from both buffers atomically. Refused bucket interactions do not dump the bucket into the world.

Pipe visuals remain simple static geometry: branching flow does not yet create original connected arm/casing models. There is no pressure simulation, source-fluid extraction, hose pulley, valve, pump-series propagation, multiblock tank merging or exact original fluid behavior. These code paths compile and pass conservation tests, but have not been exercised in a running game.
