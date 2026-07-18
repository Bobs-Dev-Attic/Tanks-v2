# Tanks-v2

A **mobile & web friendly 3D isometric WWII tank game** where you command and
control a squad of low-poly tanks across a large, varied battlefield. Built with
the [Bevy](https://bevyengine.org/) game engine and compiled to WebAssembly for
deployment on [Vercel](https://vercel.com/).

The current version is shown on the loading screen and in the HUD, and is sourced
from `Cargo.toml` (see [`CHANGELOG.md`](CHANGELOG.md)).

## Features

- **3D isometric view** using an orthographic camera at a true isometric tilt.
- **Rotate, zoom, and pan** the camera — with both mouse/keyboard and touch.
- **Low-poly WWII tanks** assembled procedurally from primitives, with
  **animated tracks** (scrolling tread texture + spinning road wheels) whose
  speed follows each tank's ground speed.
- **Varying terrain**: a procedural fractal-noise heightmap, flat-shaded for a
  faceted low-poly look, coloured from grass through dirt and rock to snow.
- **Realistic-feeling physics**: hand-written suspension with four-point terrain
  sampling, gravity, momentum, and traction, so hulls pitch and roll over the
  ground.
- **Squad command & control**: select tanks (click / tap / drag-box) and issue
  move orders (right-click / tap the ground); tanks steer into a formation.
- **Versioned loading screen** (HTML shell + in-engine), single source of truth.

## Controls

| Action           | Desktop                          | Mobile                |
| ---------------- | -------------------------------- | --------------------- |
| Select tank      | Left-click                       | Tap                   |
| Box select       | Left-drag                        | —                     |
| Move order       | Right-click ground               | Tap ground (with sel) |
| Orbit camera     | Middle-drag, or `Q` / `E`        | One-finger drag       |
| Pitch camera     | `R` / `F`                        | (part of orbit)       |
| Zoom             | Mouse wheel, or `Z` / `X`        | Pinch                 |
| Pan              | `WASD` / arrow keys              | Two-finger drag       |

## Running locally

```bash
# one-time setup
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.100

# build the static web bundle into ./dist
./build-web.sh

# serve it (any static server works)
python3 -m http.server --directory dist 8080
# open http://localhost:8080
```

You can also run it natively for quick iteration (needs system graphics/audio
libraries installed): `cargo run`.

## Deploying to Vercel

This repo deploys via Vercel's **Git integration serving a prebuilt bundle** —
the compiled `dist/` is committed, and [`vercel.json`](vercel.json) sets an empty
build command so Vercel just serves the static files. This keeps the heavy Rust
build off Vercel's builder entirely, so deploys are fast and reliable.

To update the deployed game:

```bash
./build-web.sh          # recompiles wasm into ./dist (with version injected)
git add dist && git commit -m "Rebuild web bundle" && git push
```

Vercel redeploys automatically on push. Production comes from the default branch;
other branches get preview deployments.

> A CI-built alternative is included at
> [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml): it builds the
> wasm on GitHub's runners and deploys a prebuilt bundle via the Vercel CLI
> (needs `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID` secrets). Use this
> if you'd rather not commit `dist/`.

## Project layout

```
src/
  main.rs      app wiring, window/canvas, lighting, game states
  version.rs   version string (from Cargo.toml)
  input.rs     unified mouse + touch -> GameInput
  camera.rs    isometric orbit / zoom / pan camera
  terrain.rs   procedural heightmap mesh + height/normal sampling
  tank.rs      procedural tank models + tread animation
  physics.rs   suspension + terrain-following vehicle integration
  squad.rs     selection, move orders, formation steering, markers
  ui.rs        loading screen (with version) + HUD
web/index.html HTML shell with a branded, versioned loading overlay
build-web.sh   wasm build + wasm-bindgen + static assembly
```
