# Changelog

All notable changes to **Tanks-v2** are recorded here. The version shown on the
loading screen comes from `Cargo.toml` (`version` field) via `env!("CARGO_PKG_VERSION")`.

## [0.8.0] - 2026-07-19

### Added
- **Enemies shoot back.** The German panzers now acquire the player, traverse
  their turrets and lay their guns on him (using the same terrain-aware ballistic
  solver), and fire back on a reload cadence within range. Their aim has a little
  spread so they miss more at distance, and their shells hit softer than yours, so
  a few tanks firing back stays survivable. Enemy hits damage the player through
  the existing damage system — the hull chars, mobility and gunnery degrade, and a
  knocked-out player brews up like any other tank.

## [0.7.6] - 2026-07-19

### Changed
- **Target indicator reworked to a tighter reverse-shockwave bubble.** It's
  smaller now, and collapses from a wide ring all the way down to the exact
  clicked point (instead of stopping at a fixed inner size).

### Removed
- **Snap-to-nearby-enemy targeting.** Designating now uses the exact ground point
  under the cursor/tap — no more snapping the aim to a nearby tank.

## [0.7.5] - 2026-07-19

### Changed
- **Tripled the main gun's range.** The shell's muzzle velocity was raised so its
  ballistic reach is roughly three times as far (max flat-ground range ~270 →
  ~810 world units), letting you engage targets clear across the battlefield. The
  ballistic solver uses the same value, so aiming stays accurate.

## [0.7.4] - 2026-07-19

### Changed
- **The target indicator is bigger, brighter, and zoom-aware.** It now sizes
  itself to a fraction of the camera's view, so it stays large and prominent when
  you're zoomed out (instead of shrinking to a speck), and its ring glows more
  vividly.

## [0.7.3] - 2026-07-19

### Fixed
- **The gun now compensates for terrain far more accurately.** The firing
  elevation used to be solved once, the instant you designated — before the
  turret had traversed — so the launch point (and its height over the ground) was
  wrong and the shell missed over uneven terrain. It's now re-solved from the
  live muzzle once the turret is on target, so the solution matches the geometry
  at the moment the shell actually leaves the barrel. The ballistic solver is also
  sharper: a finer trajectory integration that interpolates the exact terrain
  impact, plus refinement passes that narrow in on the best angle.

## [0.7.2] - 2026-07-19

### Fixed
- **The drive-stick area is no longer a dead zone for targeting.** The on-screen
  move stick used to claim any touch in the entire lower-left 42%×70% of the
  screen, so you couldn't designate a target there on mobile. Its touch zone is
  now sized to the visible stick base in the bottom-left corner, leaving the rest
  of the screen free to tap/​double-tap for targets.

## [0.7.1] - 2026-07-19

### Changed
- **Target indicator now sits on the ground surface**: it's a bright ring painted
  flat on the terrain under the aim point, with a low translucent shockwave dome
  over it, pinned to the ground (no more bubble floating above the target).

### Fixed
- **Designating near an enemy now targets that enemy**: clicking or double-tapping
  close to an enemy tank snaps the aim to it (its base on the ground) instead of
  the ground behind it that the ray happened to hit — so double-tap-to-fire near a
  panzer reliably engages it.

## [0.7.0] - 2026-07-19

### Added
- **Damage system**: tanks now have armor/health. Shell hits subtract HP, flash
  the hull, and progressively **char the armor**. A tank's **abilities fade with
  its condition** — a battered crew traverses the turret, reloads, and drives
  slower. A destroyed tank is knocked out: its **turret slews askew**, it brews
  up, and it keeps **smoldering and smoking** as persistent wreckage.
- **Tank collisions**: enemy tanks are solid now. Driving into a parked panzer
  stops you instead of clipping through it (parked enemies are immovable).

### Changed
- **Enemy tanks now resemble German WWII battle tanks** (Tiger/Panzer look): a
  boxy slab turret, a long overhanging gun, a commander's cupola, a vertical bow
  plate, and Schürzen side skirts, in dark Panzergrau.
- **Target indicator** is now a **reverse shockwave**: a translucent bubble that
  collapses inward toward the aim point (instead of a flat ring), then releases
  and fades when the shell is away.

## [0.6.2] - 2026-07-19

### Added
- **Enemy panzers**: a few dark-gray (Panzergrau) enemy tanks now sit downrange
  as targets to shoot at. They're static — their turret and gun are visual only.
- **Gun smoke**: the main gun now puffs out drifting smoke from the muzzle after
  it fires, on top of the muzzle flash.

### Changed
- **Target indicator** now also **pulses vertically** (bobbing up in step with
  its inward pulse) while the gun lays, and instead of vanishing the instant the
  shell leaves, it **swells, rises, and fades out** after the shot.
- **Mobile**: the on-screen **FIRE button is gone** — double-tapping the
  battlefield designates and fires the main gun, so the button was redundant.

## [0.6.1] - 2026-07-19

### Added
- **Mobile pinch-to-zoom**: a two-finger pinch now steps the zoom levels.
- **Mobile double-tap to designate**: double-tapping the battlefield selects the
  main-gun target (equivalent to a click on desktop).

## [0.6.0] - 2026-07-19

### Changed
- **The turret no longer follows the mouse.** You *designate* a target by
  clicking (or `E`); the turret then traverses to it, lays the gun, and fires
  when ready. It holds its position otherwise.
- **Better fire solution over terrain**: the firing elevation is now found by
  simulating the shell's trajectory against the actual heightfield and picking
  the angle whose impact lands closest to the target (clears or clips hills
  correctly) instead of a flat-ground formula.
- **Target indicator** is now a ring that **pulses inward toward the center** of
  the target while the gun lays.

### Added
- **Track dust**: driving kicks up dust behind the tracks, tinted by the ground
  material (grass / dirt / rock / snow) and scaled by speed.
- **Track marks**: the tank leaves persistent tracks on the ground as it drives
  (recycled with the wreckage budget).

## [0.5.1] - 2026-07-19

### Changed
- The **machine gun is now hull-mounted at the front** of the tank (co-driver's
  MG) instead of firing from the turret gun. It fires forward within an arc and
  has **limited range** (tracers fade out after ~55 units).
- Fixed the hull's front/back: the glacis and headlights now sit at the front
  (the way the tank drives and the gun points at rest), with the engine deck and
  exhausts at the rear.

## [0.5.0] - 2026-07-19

### Added
- **Target marker**: requesting a main-gun shot drops a ring marker on the aimed
  ground point; it disappears the moment the shell leaves the barrel. The gun
  also commits to that point while laying, so the shell lands where you clicked.
- **Smoldering craters**: shell impacts leave a scorched crater that glows and
  smokes for a while, then cools to a dark scar that stays.
- **Persistent wreckage**: debris and craters are no longer despawned on a timer
  — they linger as scenery, recycled only when a fixed budget (600) is exceeded.
- **Smoking debris**: about half of each blast's shards trail smoke for a few
  seconds.
- **10 discrete zoom levels** (was effectively a few), stepped by the wheel or
  `Z`/`X`, with a smooth ease between levels.

### Changed
- Projectiles (shell, tracers) and the crater/ember are now polygons, not
  spheres — polygons everywhere unless gradient circles are requested.

## [0.4.0] - 2026-07-19

### Added
- **Muzzle flashes** built from star polygons — a small one per machine-gun round
  and a big flash (plus fire wisps) for the main gun.
- **Firing feedback**: the gun recoils and the whole hull shakes when the main
  gun fires.
- **Greater tank detail**: sloped glacis, engine deck, fenders, headlights,
  exhausts, turret bustle, commander's hatch, antenna, and a muzzle brake, with
  gradient-shaded armor (vertex-colour gradients) for depth.
- **Realistic running gear**: drive sprocket, idler, road wheels, and return
  rollers that spin at radius-appropriate rates, plus cleated track links that
  march around the bottom run and wrap, alongside the scrolling track band.

### Changed
- Explosions, fire, smoke, and dust are now camera-facing **polygons with vertex
  gradients** that rotate and fade (no round sprites); debris are angular shards.
- **Wider zoom range** with smooth multiplicative zoom (much closer and much
  farther than before).

## [0.3.0] - 2026-07-19

### Changed
- **Realistic gun laying.** The turret now traverses toward the aim point at a
  fixed rate (per-tank, and scaled by the vehicle's `condition` so a damaged tank
  is slower), instead of snapping. The main gun auto-elevates to the ballistic
  angle that best hits the target (45° / max range when out of reach).
- **Fire is a request.** Pressing the main-gun fire button *requests* a shot; the
  gun fires only once it is loaded, the turret is on target, and the gun is laid
  at the right elevation. Shells fire along the actual gun line and arc to the
  target under gravity.
- The gun elevates on a separate mount from the turret's traverse.

## [0.2.1] - 2026-07-19

### Added
- **Mobile touch controls**: an on-screen left thumb-stick drives, dragging the
  right side of the screen aims the turret, and FIRE / MG buttons fire the main
  gun and machine gun. The controls appear the first time you touch the screen.
- Unified input: keyboard/mouse and touch both feed one drive/aim/fire path.

### Changed
- On-screen instructions updated to cover both keyboard/mouse and touch.

## [0.2.0] - 2026-07-18

### Added
- **Mouse-aimed turret**: the turret yaws to follow the cursor's point on the ground.
- **Two weapons**: main gun (E / left-click) fires a shell; machine gun
  (Q / right-click) sprays tracers.
- **Main-gun impact effects**: a flash, flying debris that falls and settles,
  dust and rising smoke, and a glowing ember that smolders and dims out over a
  few seconds. MG rounds kick up small dust puffs.
- Larger battlefield (map roughly doubled).

### Changed
- `Q`/`E` now fire weapons (were camera orbit); orbit is middle-drag / touch,
  pitch is `R`/`F`, zoom is the wheel / `Z`/`X`.

### Fixed
- Steering was reversed: `A`/`D` (and ←/→) now turn the expected direction.

## [0.1.4] - 2026-07-18

### Changed
- The first mission is now a **training mission**: a single tank the player
  drives directly with the keyboard (W/S or ↑/↓ to accelerate/reverse, A/D or
  ←/→ to steer). The camera follows the tank; orbit/pitch/zoom still work.
- The tank spawns resting on the ground instead of dropping in from above.

### Fixed
- Road wheels now roll about their axle (the cylinder's central axis) instead
  of tumbling about a radial axis.

## [0.1.3] - 2026-07-18

### Fixed
- Cache busting for the web bundle. `tanks.js` and `tanks_bg.wasm` have stable
  filenames but were served with a one-year `immutable` cache, so returning
  visitors could keep running an old build. The loader now appends a `?v=<version>`
  query to both, so each release fetches fresh assets.

## [0.1.2] - 2026-07-18

### Fixed
- Loading screen no longer hangs on the web. Bevy's winit backend uses an
  exception for control flow on wasm, so the JS `init()` promise never resolves
  and the HTML overlay was never dismissed. The engine now hides the overlay
  itself once it reaches the Playing state, with a JS failsafe as backup.

## [0.1.1] - 2026-07-18

### Changed
- Deployment now serves a committed prebuilt WebAssembly bundle via Vercel's Git
  integration (no Rust build on Vercel's side), making deploys fast and reliable.
- Continuous delivery: pushes to `main` publish to production; version is bumped
  on every update.

## [0.1.0] - 2026-07-18

Initial playable vertical slice.

### Added
- 3D isometric battlefield rendered with the Bevy engine, WebGL2 backend for the
  web, deployable as static files to Vercel.
- Orbit camera with rotate, zoom, and pan — driven by both mouse and touch, so it
  works on desktop and mobile.
- Procedural low-poly heightmap terrain with varying elevation and flat-shaded
  faceted look, plus a height-sampling API used by physics and orders.
- A squad of low-poly WWII-style tanks assembled from primitives, each with
  scrolling tread animation whose speed follows the tank's ground speed.
- Custom suspension physics: 4-point terrain raycasts, gravity, momentum,
  traction, and hull pitch/roll that hugs the terrain.
- Squad command & control: box-select or tap to select tanks, right-click / tap
  the ground to issue move orders, with spread-out formation targeting.
- Loading screen (HTML shell + in-engine state) that displays the current
  version, and an in-game HUD with controls help.
