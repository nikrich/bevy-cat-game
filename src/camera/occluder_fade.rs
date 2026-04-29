//! Camera-occlusion fade: any prop or placed building that sits between the
//! camera and the player gets its materials swapped to a translucent variant
//! so the cat stays visible behind trees and walls. Fades in and out
//! smoothly so the transition isn't jarring.
//!
//! Procedural meshes (most buildings, some props) live as `Mesh3d` on the
//! root entity. Kenney glTF props live as `SceneRoot` with materials nested
//! several entities deep. The walk handles both -- it descends from the
//! occluder root and fades every `MeshMaterial3d<StandardMaterial>` it
//! finds.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::building::PlacedBuilding;
use crate::player::Player;
use crate::world::props::{Prop, PropKind};

use super::GameCamera;

/// How close (in metres) an occluder's centre must be to the camera-to-player
/// line before it counts as occluding. Generous so clustered trees behind
/// the immediate canopy also fade -- their bases offset 1-2m from the line
/// would be missed at a tighter radius.
const OCCLUDE_RADIUS: f32 = 3.0;
/// Default per-mesh alpha at full fade for solid occluders (walls, single-
/// mesh props).
const OCCLUDE_ALPHA: f32 = 0.18;
/// Trees and bushes stack many canopy meshes per glTF (4-6 blobs). Each
/// blob alpha-blends, so at 0.18 a four-layer canopy still reads ~55%
/// opaque. We push tree-shaped occluders to 0.05 so the cat clearly shows
/// through.
const OCCLUDE_ALPHA_FOLIAGE: f32 = 0.05;
/// 1/seconds: at 6.0 the fade reaches near-target in ~0.5s, smoothing the
/// transition without feeling laggy.
const FADE_SPEED: f32 = 6.0;

/// Opt-out marker for occluders that should never fade -- e.g. floor and
/// roof tiles the cat stands on top of, where fading them would be confusing.
#[derive(Component)]
pub struct NoOcclude;

/// Per-entity fade progress and the descendant meshes we cloned materials
/// for. Lives in a resource to avoid query-borrow conflicts with the mesh
/// material query below.
struct FadeState {
    /// 0.0 = fully visible, 1.0 = fully transparent.
    progress: f32,
    /// Per-mesh alpha when fully faded. Trees go deeper than walls so
    /// stacked canopy layers compound to something see-through.
    full_alpha: f32,
    /// Each descendant mesh entity we swapped, with its original handle so
    /// we can restore once the fade reaches zero again.
    meshes: Vec<(Entity, Handle<StandardMaterial>)>,
}

#[derive(Resource, Default)]
pub struct OccluderFades {
    states: HashMap<Entity, FadeState>,
}

pub fn register(app: &mut App) {
    app.init_resource::<OccluderFades>()
        .add_systems(Update, fade_camera_occluders);
}

fn fade_camera_occluders(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<GameCamera>>,
    players: Query<&GlobalTransform, With<Player>>,
    occluders: Query<
        (Entity, &GlobalTransform, Option<&PropKind>),
        (Or<(With<Prop>, With<PlacedBuilding>)>, Without<NoOcclude>),
    >,
    children_q: Query<&Children>,
    mut mat_q: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fades: ResMut<OccluderFades>,
) {
    let Ok(cam_tf) = cameras.single() else { return };
    let Ok(player_tf) = players.single() else { return };

    let cam_pos = cam_tf.translation();
    let player_pos = player_tf.translation();
    let to_player = player_pos - cam_pos;
    let dist = to_player.length();
    if dist < 0.01 {
        return;
    }
    let dir = to_player / dist;
    let dt_lerp = (FADE_SPEED * time.delta_secs()).min(1.0);

    // Snapshot occluder positions, their target fade, and per-occluder
    // alpha (foliage goes deeper) for this frame so the borrow on
    // `occluders` releases before we touch material assets.
    let targets: Vec<(Entity, f32, f32)> = occluders
        .iter()
        .map(|(entity, gt, kind)| {
            // Lift the test point to the canopy/centre for tall props so
            // their actual visual mass is what we compare against the
            // camera-to-player line, not the trunk base on the ground.
            let probe_y_offset = match kind {
                Some(PropKind::Tree) => 2.0,
                Some(PropKind::PineTree) => 2.4,
                Some(PropKind::Bush) => 0.4,
                _ => 0.0,
            };
            let probe = gt.translation() + Vec3::Y * probe_y_offset;
            let to_obj = probe - cam_pos;
            let along = to_obj.dot(dir);
            let perp = (to_obj - dir * along).length();
            let occluding = along > 0.0 && along < dist && perp < OCCLUDE_RADIUS;
            let target = if occluding { 1.0 } else { 0.0 };
            let alpha = match kind {
                Some(PropKind::Tree | PropKind::PineTree | PropKind::Bush) => {
                    OCCLUDE_ALPHA_FOLIAGE
                }
                _ => OCCLUDE_ALPHA,
            };
            (entity, target, alpha)
        })
        .collect();

    let alive: std::collections::HashSet<Entity> =
        targets.iter().map(|(e, _, _)| *e).collect();

    for (root, target, full_alpha) in targets {
        let has_state = fades.states.contains_key(&root);

        if !has_state && target <= 0.0 {
            continue; // No fade and no need to start one.
        }

        if !has_state {
            // First frame this occluder is occluding: clone descendant
            // materials and start the fade-in.
            let meshes = collect_and_swap(root, &children_q, &mut mat_q, &mut materials);
            if meshes.is_empty() {
                continue;
            }
            fades.states.insert(
                root,
                FadeState {
                    progress: 0.0,
                    full_alpha,
                    meshes,
                },
            );
        }

        let state = fades.states.get_mut(&root).unwrap();
        state.progress += (target - state.progress) * dt_lerp;
        state.progress = state.progress.clamp(0.0, 1.0);

        // Done fading out: restore originals, drop state.
        if target <= 0.0 && state.progress < 0.005 {
            for (entity, original) in &state.meshes {
                if let Ok(mut mh) = mat_q.get_mut(*entity) {
                    mh.0 = original.clone();
                }
            }
            fades.states.remove(&root);
            continue;
        }

        // Push current alpha to each cloned material instance.
        let alpha = 1.0 - state.progress * (1.0 - state.full_alpha);
        for (entity, _) in &state.meshes {
            let Ok(mh) = mat_q.get(*entity) else { continue };
            if let Some(mat) = materials.get_mut(&mh.0) {
                let mut linear = mat.base_color.to_linear();
                linear.alpha = alpha;
                mat.base_color = Color::from(linear);
                mat.alpha_mode = AlphaMode::Blend;
            }
        }
    }

    // Garbage-collect entries whose entity is no longer an occluder (e.g.
    // a tree that moved out of range last frame and a chunk that despawned
    // its kids before we got here).
    fades.states.retain(|entity, _| alive.contains(entity));
}

/// Walk every descendant of `root` (and `root` itself) that owns a
/// `MeshMaterial3d<StandardMaterial>`, clone its material and swap the
/// handle. Returns the (entity, original_handle) pairs so the caller can
/// store them for later restoration.
fn collect_and_swap(
    root: Entity,
    children_q: &Query<&Children>,
    mat_q: &mut Query<&mut MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<(Entity, Handle<StandardMaterial>)> {
    let mut faded: Vec<(Entity, Handle<StandardMaterial>)> = Vec::new();
    let mut stack = vec![root];

    while let Some(entity) = stack.pop() {
        if let Ok(mut mesh_mat) = mat_q.get_mut(entity) {
            let original = mesh_mat.0.clone();
            if let Some(base) = materials.get(&original) {
                let mut translucent = base.clone();
                translucent.alpha_mode = AlphaMode::Blend;
                let new_handle = materials.add(translucent);
                mesh_mat.0 = new_handle;
                faded.push((entity, original));
            }
        }
        if let Ok(children) = children_q.get(entity) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    faded
}
