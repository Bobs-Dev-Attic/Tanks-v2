# Changelog

All notable changes to **Tanks-v2** are recorded here. The version shown on the
loading screen comes from `Cargo.toml` (`version` field) via `env!("CARGO_PKG_VERSION")`.

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
