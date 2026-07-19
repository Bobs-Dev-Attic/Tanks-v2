//! Procedural low-poly battlefield.
//!
//! A heightmap is generated from fractal value noise, turned into a faceted
//! (flat-shaded) mesh with per-vertex colours for grass / dirt / rock / snow,
//! and exposed through the [`Terrain`] resource so the physics and command
//! systems can sample ground height and slope anywhere on the map.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

/// The map spans this many world units on X and Z (centered on the origin).
pub const MAP_SIZE: f32 = 560.0;
/// Number of grid cells per side (vertices per side is `GRID + 1`).
const GRID: usize = 150;
/// Peak terrain elevation in world units.
const MAX_HEIGHT: f32 = 32.0;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain);
    }
}

/// Sampled heightmap kept around for the whole game so other systems can ask
/// "how high / how steep is the ground here?".
#[derive(Resource)]
pub struct Terrain {
    /// World extent along one axis. The map spans `[-size/2, size/2]`.
    size: f32,
    /// Vertices per side.
    verts_per_side: usize,
    /// Row-major grid of heights, `verts_per_side * verts_per_side` entries.
    heights: Vec<f32>,
}

impl Terrain {
    /// Bilinearly interpolated ground height at world `(x, z)`.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let n = self.verts_per_side;
        let half = self.size * 0.5;
        // Map world coords into grid space [0, n-1].
        let gx = ((x + half) / self.size) * (n - 1) as f32;
        let gz = ((z + half) / self.size) * (n - 1) as f32;
        let gx = gx.clamp(0.0, (n - 1) as f32);
        let gz = gz.clamp(0.0, (n - 1) as f32);

        let x0 = gx.floor() as usize;
        let z0 = gz.floor() as usize;
        let x1 = (x0 + 1).min(n - 1);
        let z1 = (z0 + 1).min(n - 1);
        let tx = gx - x0 as f32;
        let tz = gz - z0 as f32;

        let h00 = self.heights[z0 * n + x0];
        let h10 = self.heights[z0 * n + x1];
        let h01 = self.heights[z1 * n + x0];
        let h11 = self.heights[z1 * n + x1];

        let top = h00 + (h10 - h00) * tx;
        let bot = h01 + (h11 - h01) * tx;
        top + (bot - top) * tz
    }

    /// Approximate surface normal from central differences of the heightmap.
    /// Handy for gameplay/AI queries even though the physics suspension does its
    /// own four-point sampling.
    #[allow(dead_code)]
    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let e = self.size / self.verts_per_side as f32;
        let hl = self.height_at(x - e, z);
        let hr = self.height_at(x + e, z);
        let hd = self.height_at(x, z - e);
        let hu = self.height_at(x, z + e);
        Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize()
    }

    /// A reasonable spawn point around the middle of the map.
    pub fn spawn_area_center(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// Approximate surface colour (grass / dirt / rock / snow) at a world point —
    /// used to tint track dust and marks by the ground material.
    pub fn ground_color(&self, x: f32, z: f32) -> Vec3 {
        let h = self.height_at(x, z);
        let n = self.normal_at(x, z);
        let slope = (1.0 - n.y).clamp(0.0, 1.0) * 3.0;
        let c = terrain_color(h, slope);
        Vec3::new(c[0], c[1], c[2])
    }
}

fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let n = GRID + 1;
    let mut heights = vec![0.0f32; n * n];
    let half = MAP_SIZE * 0.5;

    for z in 0..n {
        for x in 0..n {
            let wx = (x as f32 / GRID as f32) * MAP_SIZE - half;
            let wz = (z as f32 / GRID as f32) * MAP_SIZE - half;
            heights[z * n + x] = terrain_height(wx, wz);
        }
    }

    let terrain = Terrain {
        size: MAP_SIZE,
        verts_per_side: n,
        heights: heights.clone(),
    };

    let mesh = build_mesh(&heights, n);
    let mesh_handle = meshes.add(mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::default(),
        Name::new("Terrain"),
    ));

    commands.insert_resource(terrain);
}

/// Faceted heightmap mesh: an indexed grid duplicated into independent
/// triangles so `compute_flat_normals` gives the low-poly look.
fn build_mesh(heights: &[f32], n: usize) -> Mesh {
    let half = MAP_SIZE * 0.5;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * n);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * n);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n * n);

    for z in 0..n {
        for x in 0..n {
            let wx = (x as f32 / (n - 1) as f32) * MAP_SIZE - half;
            let wz = (z as f32 / (n - 1) as f32) * MAP_SIZE - half;
            let h = heights[z * n + x];
            positions.push([wx, h, wz]);
            uvs.push([x as f32 / (n - 1) as f32, z as f32 / (n - 1) as f32]);

            // Slope estimate for colouring.
            let hl = heights[z * n + x.saturating_sub(1)];
            let hr = heights[z * n + (x + 1).min(n - 1)];
            let hd = heights[z.saturating_sub(1) * n + x];
            let hu = heights[(z + 1).min(n - 1) * n + x];
            let slope = ((hl - hr).abs() + (hd - hu).abs()) * 0.5;
            colors.push(terrain_color(h, slope));
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((n - 1) * (n - 1) * 6);
    for z in 0..n - 1 {
        for x in 0..n - 1 {
            let i = (z * n + x) as u32;
            let right = i + 1;
            let down = i + n as u32;
            let down_right = down + 1;
            // Two triangles, wound counter-clockwise when viewed from above.
            indices.extend_from_slice(&[i, down, right]);
            indices.extend_from_slice(&[right, down, down_right]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    // Break vertex sharing then compute per-face normals for the faceted look.
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

/// Colour ramp from grass through dirt and rock up to snow, biased by slope.
fn terrain_color(h: f32, slope: f32) -> [f32; 4] {
    let t = (h / MAX_HEIGHT).clamp(0.0, 1.0);
    let grass = Vec3::new(0.28, 0.44, 0.18);
    let dirt = Vec3::new(0.42, 0.34, 0.22);
    let rock = Vec3::new(0.38, 0.38, 0.41);
    let snow = Vec3::new(0.90, 0.92, 0.96);

    let mut c = if t < 0.4 {
        grass.lerp(dirt, (t / 0.4).clamp(0.0, 1.0))
    } else if t < 0.75 {
        dirt.lerp(rock, ((t - 0.4) / 0.35).clamp(0.0, 1.0))
    } else {
        rock.lerp(snow, ((t - 0.75) / 0.25).clamp(0.0, 1.0))
    };

    // Steep faces trend rocky regardless of altitude.
    let steep = (slope * 0.6).clamp(0.0, 1.0);
    c = c.lerp(rock, steep);
    [c.x, c.y, c.z, 1.0]
}

/// Fractal value-noise terrain with a broad basin in the middle so the squad
/// has open ground to manoeuvre, and taller ridges toward the edges.
fn terrain_height(x: f32, z: f32) -> f32 {
    let mut amplitude = 1.0;
    let mut frequency = 1.0 / 90.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..5 {
        sum += value_noise(x * frequency, z * frequency) * amplitude;
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    let base = (sum / norm) * 0.5 + 0.5; // 0..1

    // Flatten the central arena, raise the rim.
    let dist = (x * x + z * z).sqrt() / (MAP_SIZE * 0.5);
    let bowl = smoothstep(0.25, 1.0, dist);
    let h = base * (0.35 + 0.65 * bowl);
    h * MAX_HEIGHT
}

/// Deterministic 2D value noise with smooth interpolation. No external deps so
/// the wasm build stays lean and reproducible.
fn value_noise(x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let xf = x - xi;
    let zf = z - zi;

    let v00 = hash2(xi, zi);
    let v10 = hash2(xi + 1.0, zi);
    let v01 = hash2(xi, zi + 1.0);
    let v11 = hash2(xi + 1.0, zi + 1.0);

    let u = fade(xf);
    let v = fade(zf);
    let top = v00 + (v10 - v00) * u;
    let bot = v01 + (v11 - v01) * u;
    (top + (bot - top) * v) * 2.0 - 1.0 // -1..1
}

fn hash2(x: f32, z: f32) -> f32 {
    let mut h = (x * 127.1 + z * 311.7).sin() * 43758.547;
    h -= h.floor();
    h
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
