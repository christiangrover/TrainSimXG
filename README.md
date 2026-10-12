# Table Top Trains

A toy train set on a 100 × 100 table, built in Rust with [Bevy](https://bevyengine.org) 0.16 and egui.
Lay track, sculpt hills, build towns and roads, then watch the trains run from beside them, from a passenger seat, or from the engineer's cab.

```
cargo run --release
```

The game opens with a demo layout. **New… → Yes, clear** gives you an empty table.

## Views (keys 1–5)

| Key | View | Controls |
|---|---|---|
| 1 | Overview | WASD/arrows pan, Q/E or right-drag rotate, mouse wheel zoom |
| 2 | Walk around the table | WASD walk, right-drag look, mouse wheel crouch or stand, Shift run |
| 3 | Follow train | right-drag orbit, mouse wheel zoom |
| 4 | Passenger | right-drag look, Tab next car, V other side's window |
| 5 | Engineer | W/Up throttle up, S/Down throttle down (below zero is reverse), X shut off, hold Space brake |

T cycles through the trains. You can also pick one in the right-hand panel.

## Building

Choose a tool in the left toolbox, then left-click on the table. Drag to paint roads, terrain and scenery.

- **R** rotates the piece. A cyan preview shows the rails before you place them.
- **[ ]** changes the brush size for terrain tools. **Esc** goes back to the Select tool.
- **Track:** straight, curve, left and right switches, diamond crossing, ramp, station, and road crossing (road crossings have gates that lower when a train comes).
- **Heights:** new track automatically matches the height of the track next to it. A ramp climbs one level toward its arrow. Use *Raise track* and *Lower track* to adjust a placed piece.
  - Track above the ground is drawn as a **bridge** with piers.
  - Track below the ground is a **tunnel**, with stone portals where it comes out.
- **Switches:** click one with the Select tool to flip it. Green means straight, orange means diverging.
- **Trains:** build a train in the *Train library*, click *Place on track…*, then click on any track. The library has:
  - **Engines:** a steam engine, a diesel road engine and a diesel streamliner. Steam engines puff white smoke; diesels give dark exhaust, heaviest when pulling away.
  - **Cars:** coal tender, passenger, sleeper, dining, boxcar, tanker, coal hopper, log car and caboose.
- **Auto trains** cruise, stop at stations, wait for trains ahead, and reverse at the end of the line. **Manual trains** are driven with the throttle, either from the panel or from the engineer's view.
- **Road traffic:** cars, vans, trucks and buses drive on every road. They keep a safe gap, take turns at junctions, and wait at closed crossing gates. Set how many with the slider.
- **Emergency services:** a *Police station* sends out 2 police cars, a *Hospital* 2 ambulances, and a *Fire station* a fire engine, onto the nearby roads. Now and then they go out on a call, with flashing light bars and a faster speed. Remove the building and its vehicles go too.

## Saving

Type a name in the top bar and press **Save**. Layouts, including terrain, every piece and every train, are saved as JSON in `saves/`. Load them again from the **Load…** menu.

## Running it on another Windows PC

The game is a single `.exe` with no installer and nothing else to install. The models are generated in code, the textures are embedded in the exe, and so is the C runtime.

- **Build the zip yourself:** run `.\package.ps1`. It creates `dist\TableTopTrains-windows-x64.zip`. Copy the zip to the other PC, unzip it anywhere, and double-click `TableTopTrains.exe`.
- **Let GitHub build it:** push a version tag (`git tag v0.1.0` then `git push origin v0.1.0`). The *Release* workflow tests the code, builds the zip and attaches it to a GitHub Release, ready to download.

Saved layouts go in a `saves` folder next to the exe. Windows SmartScreen may warn about an unsigned app the first time you run it; click **More info → Run anyway**. The game needs a graphics card with DirectX 12 or Vulkan, which any Windows 11 PC has.

## Textures

The photo textures (grass, gravel, asphalt, brick, roof tiles, wood and more) are CC0 public-domain textures from [Poly Haven](https://polyhaven.com). They are free to use with no licence fees. See [assets/textures/CREDITS.md](assets/textures/CREDITS.md).

## Code map

| File | Contents |
|---|---|
| `grid.rs` | tiles, track pieces, how edges connect |
| `track.rs` | rail geometry and moving along the track |
| `trains.rs` | train simulation, smoke |
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
