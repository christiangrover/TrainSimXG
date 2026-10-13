# Table Top Trains

A toy train set on a 100 × 100 table, built in Rust with [Bevy](https://bevyengine.org) 0.16 and egui.
Lay track, sculpt hills, build towns and roads, then watch the trains run from beside them, from a passenger seat, or from the engineer's cab.

```
cargo run --release
```

The game opens with a demo layout. **New… → Yes, clear** gives you an empty table.

## Views (keys 1–7)

| Key | View | Controls |
|---|---|---|
| 1 | Overview | WASD/arrows pan, Q/E or right-drag rotate, mouse wheel zoom |
| 2 | Walk around the table | WASD walk, right-drag look, mouse wheel crouch or stand, Shift run |
| 3 | Follow train | right-drag orbit, mouse wheel zoom |
| 4 | Passenger | right-drag look, Tab next car, V other side's window |
| 5 | Engineer | Drive from the cab: W/S throttle (below STOP is reverse), X shut off, hold Space to brake, E emergency brake, H horn. Or use the throttle and brake levers and the buttons on the control desk. |
| 6 | Fireman | Stand on a steam engine's footplate. Press Space (or SHOVEL COAL) to throw coal into the firebox and keep the steam needle in the green. |
| 7 | Crane | Drive the port crane: A/D swing, W/S trolley out and in, R/F hook up and down, Space grab or drop, Tab switches between the hook (logs) and the grab bucket (rocks), C next crane. A green ring or box shows when you can grab or drop, yellow means lower it more, red means the wrong tool or car. |

T cycles through the trains. You can also pick one in the right-hand panel. H sounds the horn (or the whistle on a steam engine) from any train view.

## Stations and passengers

People wait on every platform and stroll about. When a passenger train stops, it pulls up with its coaches along the platform. Some passengers get off and walk into the ticket office, and the people waiting walk to the carriage doors and get on. Self-driving trains wait until everyone is aboard. You can see passengers sitting inside through the coach windows, and the train panel shows how many are on board.

Lay several station pieces in a row for a longer platform; it gets one ticket office in the middle. Passenger trains don't stop at goods stations (stations next to a port).

## Fuel, coal and water

Each train has a **Fuel: Auto / Manual** switch, in the train panel and on the cab desk.
- **Auto:** engines never run out.
- **Manual:**
  - Diesels burn fuel.
  - Steam engines burn coal and use water, and their fire burns down. Boiler pressure is what moves a steam engine, so on Manual you are the fireman: shovel coal in view 6 to keep the pressure up. Too much coal makes black smoke. Self-driving steam trains have their own fireman.
  - Stopping at a station tops up fuel, coal and water. Water towers refill steam engines, and fuel depots refill diesels.

## Ports, cranes and cargo

Freight cars carry real loads that show on the train: logs on log cars, rocks in hoppers, and water (blue band) or oil (black band) in tank cars.
- **Port with crane:** goes beside water. It has a quay, a moored ship, and piles of logs and rocks. Lay track next to it and add a station so self-driving trains stop there. The crane loads stopped trains automatically, and the train waits until it's done. Press 7 to drive the crane yourself.
- **Water tower / Fuel depot:** placed beside the track. A tank car stopped next to one fills with water or oil.
- **Unloading:** cargo unloads when the train stops at any other station, so there's always more to do.

## Building

Choose a tool in the left toolbox, then left-click on the table. Drag to paint roads, terrain and scenery.

- **R** rotates the piece. A cyan preview shows the rails before you place them.
- **[ ]** changes the brush size for terrain tools. **Esc** goes back to the Select tool.
- **Track:** straight, curve, left and right switches, diamond crossing, ramp, station, and road crossing (road crossings have gates that lower when a train comes).
- **Heights:** new track automatically matches the height of the track next to it. A ramp climbs one level toward its arrow. Use *Raise track* and *Lower track* to adjust a placed piece: the straight track on either side turns into ramps by itself, one level per tile, so trains can still get through. Raise the pieces next to it too, or drag the tool along the line, to build a long, flat bridge; a sloping piece levels off instead of making a peak. Curves, switches and stations can't slope, so leave straight track around the piece you raise.
  - Track above the ground is drawn as a **bridge** with piers.
  - Track below the ground is a **tunnel**, with stone portals where it comes out.
  - **Track over track:** lay straight track across another line to cross it on the level (a diamond crossing). Click the crossing with *Raise track* to lift one line onto a bridge over the other; the approaches slope up by themselves. Click the bridge with *Select* to swap which line goes over. You can also use the *Overpass* piece, or lay a line that's already raised straight across a lower one.
- **Switches:** click one with the Select tool to flip it. Green means straight, orange means diverging.
- **Trains:** build a train in the *Train library*, click *Place on track…*, then click on any track. The library has:
  - **Engines:** a steam engine, a diesel road engine and a diesel streamliner. Steam engines puff white smoke; diesels give dark exhaust, heaviest when pulling away.
  - **Cars:** coal tender, passenger, sleeper, dining, boxcar, tanker, coal hopper, log car and caboose.
- **Auto trains** cruise, stop at stations, wait for trains ahead, and reverse at the end of the line. **Manual trains** are driven with the throttle, either from the panel or from the engineer's view.
- **Road traffic:** cars, vans, trucks and buses drive on every road. They keep a safe gap, stop at red traffic lights, never block a junction, and wait at closed crossing gates. Set how many with the slider.
- **Traffic lights** appear by themselves at every road junction. **Level crossings** flash their red lights and ring a bell (when you're nearby) while the gates are down.
- **Emergency services:** a *Police station* sends out 2 police cars, a *Hospital* 2 ambulances, and a *Fire station* a fire engine, onto the nearby roads. Now and then they go out on a call, with flashing light bars and a faster speed. Remove the building and its vehicles go too.

## Saving

Type a name in the top bar and press **Save**. Layouts, including terrain, every piece and every train, are saved as JSON in `saves/`. Load them again from the **Load…** menu.

## Running it on another Windows PC

The game is a single `.exe` with no installer and nothing else to install. The models are generated in code, the textures are embedded in the exe, and so is the C runtime.

- **Build the zip yourself:** run `.\package.ps1`. It creates `dist\TableTopTrains-windows-x64.zip`. Copy the zip to the other PC, unzip it anywhere, and double-click `TableTopTrains.exe`.
- **Let GitHub build it:** push a version tag (`git tag v0.1.0` then `git push origin v0.1.0`). The *Release* workflow tests the code, builds the zip and attaches it to a GitHub Release, ready to download.

Saved layouts go in a `saves` folder next to the exe. Windows SmartScreen may warn about an unsigned app the first time you run it; click **More info → Run anyway**. The game needs a graphics card with DirectX 12 or Vulkan, which any Windows 11 PC has.

## Textures and lighting

The photo textures (grass, gravel, asphalt, paving, brick, roof tiles, wood, worn paint and more) come from [ambientCG](https://ambientcg.com) and [Poly Haven](https://polyhaven.com). Each material has colour, bump, roughness and shadowing maps. The scene is lit by a photo of a real sunny living room (an HDRI), which also gives the trains and windows their reflections. Everything is CC0 public domain: free, with no licence fees. See [assets/textures/CREDITS.md](assets/textures/CREDITS.md).

## Code map

| File | Contents |
|---|---|
| `grid.rs` | tiles, track pieces, how edges connect |
| `track.rs` | rail geometry and moving along the track |
| `trains.rs` | train simulation, fuel, cargo, smoke |
| `cab.rs`, `cab_ui.rs` | engineer and fireman controls, cab desk, gauges |
| `crane.rs` | port cranes: controls and automatic loading |
| `people.rs` | people on platforms, getting on and off trains |
| `envmap.rs` | room lighting from the HDRI photo |
| `services.rs` | refuelling, filling and unloading at stops |
| `sounds.rs` | sound effects, synthesised in code |
| `roads.rs` | road traffic, emergency vehicles, crossing gates |
| `build.rs` | toolbox, picking tiles |
| `camera.rs` | the five views |
| `scene.rs` | room, table, lighting, terrain and tile meshes |
| `mesh.rs`, `materials.rs` | shape builder, textures and materials |
| `models_*.rs` | procedural models |
| `ui.rs` | panels and overlays |
| `save.rs` | save and load |
| `demo.rs` | starter layout |

Run `cargo test` for the core logic tests. Two debug aids:
- `TRAINSIM_SHOTS=<dir>` captures a tour of screenshots and then exits.
- `TRAINSIM_TRAFFIC_CHECK=<runs>` soak-tests road traffic headlessly, failing on any collision or gridlock.
