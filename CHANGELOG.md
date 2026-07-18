# Changelog

All notable changes to **Tanks-v2** are recorded here. The version shown on the
loading screen comes from `Cargo.toml` (`version` field) via `env!("CARGO_PKG_VERSION")`.

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
