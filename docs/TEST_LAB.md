# Create Rust Automation Lab

This is a real saved creative world for Horizon 26.3 with the matching Create content and native support packages. It covers the implemented development subset; it does not establish completion or original Create parity. No running-game walkthrough has been performed.

For later use, extract the world folder into Horizon's configured saves directory. `level.dat` belongs directly inside `Create Rust Automation Lab/`, beside `region/`. Use a copy when resetting the tests. The generator only writes a new isolated folder and never opens existing player saves.

Spawn is (8,65,5). Creative flight reaches the elevated machine interfaces. Three chests nearby contain all 77 registered items; the fourth contains tools and consumables. A gallery behind the lab contains the 40 registered blocks. The arena has six rows of numbered bays, with origins recorded below and in `test-plan.json`.

Most drive levers start ON: ON disengages the clutch and pauses the load; OFF runs it. Motors upstream of a disengaged clutch may rotate. Wheel waterfalls are live; their output clutches start paused. The two control demos intentionally show conflict and overstress. The gearshift's second lever reverses the output. Feed levers on belts are separately paused: release drives first, then feed. The drain has a button-controlled bucket dropper. Downward droppers have input stacks and a button; press once after releasing the corresponding drives. They keep world-processing input from falling away before the test starts. Water/lava and supported fire media are already placed in fan bays. Do not remove their containing blocks during a baseline test.

Machines with interfaces use the top row for ingredients and the middle row for products. Mixing and compacting ingredients are in the basin, not in the overhead machine. The mixer burner is preheated for the baseline run; fuel is supplied nearby. Fluids are preloaded in the spout and shared-tank demo. The pump has two outlet tanks and renewable source water. The crafter wall is a complete 5x5 rectangle with two crafts of the crushing-wheel pattern, including empty corner slots. One externally driven corner powers the connected wall.

Bays 26–29 demonstrate depot and belt applications, a retained named axe, and casing production. Deployer held materials are in the deployer input row; target ingredients are in the depot or feed chest.

The world includes destructive drill/tree targets and a named deployer input that must remain unchanged. Restore the original ZIP to repeat these cases reliably. Chance-based recipes may yield different counts.

`validation.json` records headless checks: official Horizon level, region, chunk and block-entity codecs; persisted inventories and machine data; expected drive RPMs; paused loads; stress/conflict; nonzero wheel torque using Horizon's falling-fluid flow, five prefilled recipe transactions, crafter orientation/input patterns, and complete deployer application batches with held-material and tool-retention checks. These checks do not substitute for rendering, interaction, multiplayer or a running-game walkthrough.

Major unfinished port areas include moving contraptions, trains, schematics, logistics/computer systems, full original content and full rendering/behavior parity. Consult FUNCTIONAL_SCOPE.md in the source repository before treating a missing feature as a test-map failure.

Regenerate from source with `python3 tools/build_test_lab.py --horizon /path/to/pinned/horizon --output dist/create-rust-test-lab.zip`. Requires Rust and the Horizon revision pinned by the native package. The ZIP contains authored world data and identifiers, not original Create artwork or a Minecraft resource pack.
