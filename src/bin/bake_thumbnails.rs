//! One-off tool that renders a thumbnail PNG for every named node in the
//! LowPoly Interior GLB pack. Run via:
//!
//! ```
//! cargo run --release --bin bake_thumbnails
//! ```
//!
//! Output lands in `assets/thumbnails/{node_name}.png` (e.g.
//! `assets/thumbnails/armchair.008.png`). The runtime catalog UI loads
//! these via `asset_server.load()` and displays them in the decoration
//! browser. Re-run when the source GLBs change.
//!
//! Implementation notes:
//! - Uses Bevy's headless `Screenshot::image(handle)` + `save_to_disk`
//!   for GPU readback — sidesteps writing our own wgpu copy.
//! - One node is rendered per "slot" frame. A small state machine tracks
//!   load → mount → settle → capture → advance, gated by a per-node frame
//!   counter that gives the GPU time to actually draw before screenshot.
//! - Camera + light + currently-rendered item all live on a custom render
//!   layer so the (otherwise empty) main-window render doesn't flash the
//!   thumbnails.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::mesh::VertexAttributeValues;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;
use std::collections::VecDeque;
use std::path::PathBuf;

const FIRST_PATH: &str = "models/lowpoly_interior/interior_1-500.glb";
const SECOND_PATH: &str = "models/lowpoly_interior/interior_501-1000.glb";
const THUMB_SIZE: u32 = 192;
/// Frames between mounting an item and screenshot. A few frames give the
/// renderer time to extract + draw the new transform.
const FRAMES_PER_ITEM: u32 = 3;
const RENDER_LAYER: usize = 1;

#[derive(Resource)]
struct BakeContext {
    first_handle: Handle<Gltf>,
    second_handle: Handle<Gltf>,
    target_image: Handle<Image>,
    queue: VecDeque<BakeItem>,
    state: BakeState,
    /// Frames the current item has been visible to the renderer.
    frames_in_state: u32,
    /// Item entity currently mounted in front of the camera, if any.
    current_item: Option<Entity>,
    output_dir: PathBuf,
    total_baked: u32,
}

struct BakeItem {
    name: String,
    /// Which GLB the node lives in (`true` = first, `false` = second).
    first: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum BakeState {
    LoadingGltf,
    BuildingQueue,
    Mounting,
    Settling,
    Captured,
    Done,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Cat Game — thumbnail baker".into(),
                resolution: WindowResolution::new(THUMB_SIZE * 2, THUMB_SIZE * 2),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, drive_bake)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let first_handle = asset_server.load(FIRST_PATH);
    let second_handle = asset_server.load(SECOND_PATH);

    let target_image = images.add(make_target_image(THUMB_SIZE));
    let layers = RenderLayers::layer(RENDER_LAYER);

    // Offscreen render camera. Iso-style angle, looking at origin. The
    // RenderTarget component overrides the default Window target.
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.92, 0.88, 0.78, 1.0)),
            order: -1,
            ..default()
        },
        RenderTarget::Image(target_image.clone().into()),
        Projection::from(PerspectiveProjection { fov: 0.6, ..default() }),
        Transform::from_xyz(2.5, 2.0, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
    ));

    // Key + fill lights so meshes have legible shading.
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 6.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 4_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 2.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
    ));

    // Main window camera kept so winit doesn't error on a zero-camera world.
    // Parked far away with default clear so any flicker is obviously not
    // the baked output.
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.05, 0.05)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1000.0),
    ));

    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("CARGO_MANIFEST_DIR must be set when running via cargo");
    let output_dir = manifest.join("assets/thumbnails");
    std::fs::create_dir_all(&output_dir).expect("create thumbnails dir");

    info!("[bake] target dir: {}", output_dir.display());

    commands.insert_resource(BakeContext {
        first_handle,
        second_handle,
        target_image,
        queue: VecDeque::new(),
        state: BakeState::LoadingGltf,
        frames_in_state: 0,
        current_item: None,
        output_dir,
        total_baked: 0,
    });
}

fn make_target_image(size: u32) -> Image {
    // RGBA8 = 4 bytes per pixel. Hardcoded — the texture format is fixed
    // here so we don't need the round-trip through `pixel_size()`.
    let pixel_size = 4usize;
    let mut img = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("thumbnail-target"),
            size: Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        sampler: ImageSampler::default(),
        ..default()
    };
    img.data = Some(vec![0u8; (size * size) as usize * pixel_size]);
    img.asset_usage = RenderAssetUsages::all();
    img
}

#[allow(clippy::too_many_arguments)]
fn drive_bake(
    mut commands: Commands,
    mut ctx: ResMut<BakeContext>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut exits: MessageWriter<AppExit>,
) {
    match ctx.state {
        BakeState::LoadingGltf => {
            if gltfs.get(&ctx.first_handle).is_some()
                && gltfs.get(&ctx.second_handle).is_some()
            {
                ctx.state = BakeState::BuildingQueue;
            }
        }
        BakeState::BuildingQueue => {
            if let Some(g) = gltfs.get(&ctx.first_handle) {
                for name in g.named_nodes.keys() {
                    ctx.queue.push_back(BakeItem { name: name.to_string(), first: true });
                }
            }
            if let Some(g) = gltfs.get(&ctx.second_handle) {
                for name in g.named_nodes.keys() {
                    ctx.queue.push_back(BakeItem { name: name.to_string(), first: false });
                }
            }
            info!("[bake] queued {} items", ctx.queue.len());
            ctx.state = BakeState::Mounting;
            ctx.frames_in_state = 0;
        }
        BakeState::Mounting => {
            if let Some(prev) = ctx.current_item.take() {
                commands.entity(prev).despawn();
            }

            let Some(item) = ctx.queue.front() else {
                ctx.state = BakeState::Done;
                return;
            };
            let gltf_handle = if item.first { &ctx.first_handle } else { &ctx.second_handle };
            let Some(gltf) = gltfs.get(gltf_handle) else {
                warn!("[bake] gltf vanished mid-bake");
                ctx.state = BakeState::Done;
                return;
            };
            let Some(node_handle) = gltf.named_nodes.get(item.name.as_str()) else {
                warn!("[bake] node '{}' missing — skipping", item.name);
                ctx.queue.pop_front();
                return;
            };
            let Some(node) = gltf_nodes.get(node_handle) else { return };
            let Some(mesh_handle) = node.mesh.as_ref() else {
                ctx.queue.pop_front();
                return;
            };
            let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else { return };

            // Frame the asset: union all primitive AABBs (in node-local
            // post-TRS space), centre at origin, scale so the largest
            // dimension fits ~1.6 units (the camera distance is 2.5 so
            // this leaves comfortable margin in the rendered frame).
            let aabb = combined_aabb(gltf_mesh, &meshes, &node.transform);
            let size = aabb.half_extents.max_element() * 2.0;
            let scale = if size > 1e-4 { 1.6 / size } else { 1.0 };
            let centre: Vec3 = aabb.center.into();

            let parent = commands
                .spawn((
                    Transform::from_translation(-centre * scale)
                        .with_scale(Vec3::splat(scale)),
                    Visibility::Inherited,
                    RenderLayers::layer(RENDER_LAYER),
                ))
                .id();
            let local_tf = Transform {
                translation: Vec3::ZERO,
                rotation: node.transform.rotation,
                scale: node.transform.scale,
            };
            commands.entity(parent).with_children(|p| {
                for prim in &gltf_mesh.primitives {
                    let mat = prim.material.clone().unwrap_or_default();
                    p.spawn((
                        Mesh3d(prim.mesh.clone()),
                        MeshMaterial3d(mat),
                        local_tf,
                        RenderLayers::layer(RENDER_LAYER),
                    ));
                }
            });
            ctx.current_item = Some(parent);
            ctx.state = BakeState::Settling;
            ctx.frames_in_state = 0;
        }
        BakeState::Settling => {
            ctx.frames_in_state += 1;
            if ctx.frames_in_state >= FRAMES_PER_ITEM {
                let item = ctx.queue.front().expect("queue non-empty in Settling");
                let path = ctx.output_dir.join(format!("{}.png", item.name));
                let target = ctx.target_image.clone();
                commands
                    .spawn(Screenshot::image(target))
                    .observe(save_to_disk(path));
                ctx.state = BakeState::Captured;
                ctx.frames_in_state = 0;
            }
        }
        BakeState::Captured => {
            ctx.frames_in_state += 1;
            if ctx.frames_in_state >= 1 {
                ctx.queue.pop_front();
                ctx.total_baked += 1;
                if ctx.total_baked.is_multiple_of(50) {
                    info!("[bake] {} done, {} remaining", ctx.total_baked, ctx.queue.len());
                }
                ctx.state = BakeState::Mounting;
                ctx.frames_in_state = 0;
            }
        }
        BakeState::Done => {
            info!(
                "[bake] complete — {} thumbnails written to {}",
                ctx.total_baked,
                ctx.output_dir.display()
            );
            exits.write(AppExit::Success);
        }
    }
}

/// Union AABB of all primitives in the mesh, transformed by node TRS
/// rotation + scale. Translation is dropped — the scene-grid layout
/// shouldn't influence the asset's intrinsic bounds.
fn combined_aabb(
    gltf_mesh: &GltfMesh,
    meshes: &Assets<Mesh>,
    node_tf: &Transform,
) -> Aabb {
    let mut combined: Option<(Vec3, Vec3)> = None;
    for prim in &gltf_mesh.primitives {
        let Some(mesh) = meshes.get(&prim.mesh) else { continue };
        let Some((pmin, pmax)) = mesh_position_extents(mesh) else { continue };
        // Transform the 8 corners by node TRS (rotation + scale) and
        // refit. Required because rotation can produce a tighter or
        // looser AABB than just rotating min/max independently.
        let corners = [
            Vec3::new(pmin.x, pmin.y, pmin.z),
            Vec3::new(pmax.x, pmin.y, pmin.z),
            Vec3::new(pmin.x, pmax.y, pmin.z),
            Vec3::new(pmax.x, pmax.y, pmin.z),
            Vec3::new(pmin.x, pmin.y, pmax.z),
            Vec3::new(pmax.x, pmin.y, pmax.z),
            Vec3::new(pmin.x, pmax.y, pmax.z),
            Vec3::new(pmax.x, pmax.y, pmax.z),
        ];
        let mut tmin = Vec3::splat(f32::INFINITY);
        let mut tmax = Vec3::splat(f32::NEG_INFINITY);
        for c in corners {
            let p = node_tf.rotation * (c * node_tf.scale);
            tmin = tmin.min(p);
            tmax = tmax.max(p);
        }
        combined = Some(match combined {
            Some((mn, mx)) => (mn.min(tmin), mx.max(tmax)),
            None => (tmin, tmax),
        });
    }
    let (mn, mx) = combined.unwrap_or((Vec3::splat(-0.5), Vec3::splat(0.5)));
    Aabb::from_min_max(mn, mx)
}

/// Walk a `Mesh`'s POSITION attribute to find its min / max corners.
/// Bevy 0.18 dropped `Mesh::compute_aabb()` so we read the raw attribute
/// directly. Returns `None` if the mesh has no positions or uses an
/// unexpected attribute encoding.
fn mesh_position_extents(mesh: &Mesh) -> Option<(Vec3, Vec3)> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return None;
    };
    if positions.is_empty() {
        return None;
    }
    let mut mn = Vec3::splat(f32::INFINITY);
    let mut mx = Vec3::splat(f32::NEG_INFINITY);
    for p in positions {
        let v = Vec3::new(p[0], p[1], p[2]);
        mn = mn.min(v);
        mx = mx.max(v);
    }
    Some((mn, mx))
}
