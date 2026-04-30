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
use crate::items::{Form, ItemRegistry, ItemTags};
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
/// Default alpha for the indoor reveal pass. Fully transparent so the
/// roof / upper-storey contents disappear entirely when the cat is
/// indoors — reads cleanest at the default iso angle. The player can
/// override via the build-mode UI (`IndoorRevealSettings.alpha`).
const DEFAULT_INDOOR_REVEAL_ALPHA: f32 = 0.0;

/// Player-tweakable settings for the indoor reveal effect. Edited from
/// the build-mode tool palette: a toggle to disable the reveal entirely
/// (so the player can see the building exterior while building) and a
/// slider for the alpha used when ceiling pieces fade.
#[derive(Resource)]
pub struct IndoorRevealSettings {
    pub enabled: bool,
    pub alpha: f32,
}

impl Default for IndoorRevealSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            alpha: DEFAULT_INDOOR_REVEAL_ALPHA,
        }
    }
}
/// 1/seconds: at 6.0 the fade reaches near-target in ~0.5s, smoothing the
/// transition without feeling laggy.
const FADE_SPEED: f32 = 6.0;
/// XZ radius around the player to scan for "is anything overhead?". Generous
/// enough to detect a roof when the cat is near a wall but not so wide that
/// the cat triggers the indoor reveal while standing next to a building.
const INDOOR_PROBE_RADIUS: f32 = 0.7;
/// Y window above the cat's centre that counts as "overhead". `+0.3` skips
/// pieces at the same level (so a wall the cat is brushing against doesn't
/// count). The upper bound is large enough to catch the roof of a tall
/// multi-storey hall — anything reasonably above the player should
/// register, since stray floating cubes 30m up are unlikely.
const INDOOR_OVERHEAD_MIN_Y: f32 = 0.3;
const INDOOR_OVERHEAD_MAX_Y: f32 = 30.0;
/// XZ radius around the player within which overhead cubes fade once the
/// player is detected as indoors. Sized for a full 20×20 hall so an
/// entire roof reveals at once even when the player is at one corner;
/// keeping it bounded (vs. "fade everything above") means a neighbour
/// building 25m away doesn't go translucent when you step inside yours.
const INDOOR_REVEAL_RADIUS: f32 = 14.0;
/// Minimum Y offset above the player for a cube to qualify as a "ceiling"
/// piece for the indoor reveal. Below this, cubes are walls — those fade
/// via the existing camera-line rule.
const INDOOR_CEILING_MIN_OFFSET: f32 = 0.6;

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

impl OccluderFades {
    /// Whether `entity` is currently fading (or fully faded) for any reason
    /// — camera-line occlusion or the indoor ceiling reveal. Used by the
    /// placement raycast to skip see-through walls so the cursor lands on
    /// the floor / interior, not on the wall the player is looking past.
    pub fn is_faded(&self, entity: Entity) -> bool {
        self.states.contains_key(&entity)
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<OccluderFades>()
        .init_resource::<IndoorRevealSettings>()
        .add_systems(Update, fade_camera_occluders);
}

fn fade_camera_occluders(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<GameCamera>>,
    players: Query<&GlobalTransform, With<Player>>,
    occluders: Query<
        (Entity, &GlobalTransform, Option<&PropKind>, Option<&PlacedBuilding>),
        (Or<(With<Prop>, With<PlacedBuilding>)>, Without<NoOcclude>),
    >,
    registry: Res<ItemRegistry>,
    children_q: Query<&Children>,
    mut mat_q: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fades: ResMut<OccluderFades>,
    indoor_settings: Res<IndoorRevealSettings>,
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

    // Indoor detection: is there a placed building cube directly above the
    // player? If so, fade the whole ceiling layer + the camera-side walls
    // so the player can see the floor. Only structural pieces (walls,
    // doors, windows) trigger the indoor flag — a chair on a shelf above
    // shouldn't make the cat "indoors". The whole reveal can be disabled
    // by the player via the build-mode toggle.
    let indoor = indoor_settings.enabled
        && occluders.iter().any(|(_, gt, _, building)| {
            let Some(b) = building else { return false };
            let Some(def) = registry.get(b.item) else { return false };
            if !def.tags.contains(ItemTags::STRUCTURAL) {
                return false;
            }
            let pos = gt.translation();
            let dx = pos.x - player_pos.x;
            let dz = pos.z - player_pos.z;
            let dy = pos.y - player_pos.y;
            dx * dx + dz * dz < INDOOR_PROBE_RADIUS * INDOOR_PROBE_RADIUS
                && dy > INDOOR_OVERHEAD_MIN_Y
                && dy < INDOOR_OVERHEAD_MAX_Y
        });

    // Snapshot occluder positions, their target fade, and per-occluder
    // alpha (foliage goes deeper) for this frame so the borrow on
    // `occluders` releases before we touch material assets.
    let targets: Vec<(Entity, f32, f32)> = occluders
        .iter()
        .map(|(entity, gt, kind, building)| {
            let is_building = building.is_some();
            // Camera-line fade (the "I'm behind a tree / wall" case) is
            // restricted to props + structural building pieces. Furniture
            // and decorations stay solid even when between camera and
            // player — high-poly furniture looks busy at low alpha and the
            // player rarely needs to see *through* a chair to spot the
            // cat. Indoor reveal still kicks in for furniture above the
            // player when they go up a story.
            let allow_camera_line = match building {
                None => true, // Prop — keep existing behaviour.
                Some(b) => registry
                    .get(b.item)
                    .map(|d| {
                        // Structural pieces fade for camera-line occlusion
                        // *except* floors — the floor under the player
                        // would always pass the camera-line test, fading
                        // the ground out from under them. Floors still
                        // fade via the indoor ceiling pass when they're
                        // above the player.
                        d.tags.contains(ItemTags::STRUCTURAL)
                            && !matches!(d.form, Form::Floor)
                    })
                    .unwrap_or(false),
            };

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
            let camera_line_occluding = allow_camera_line
                && along > 0.0
                && along < dist
                && perp < OCCLUDE_RADIUS;

            // Indoor reveal: when the player is under a roof, fade any
            // STRUCTURAL piece (wall / floor / door / window) above the
            // player's centre within a room radius. Furniture is excluded
            // from this pass — items saved at varying Y from past
            // sessions kept landing in the "above" window and going
            // invisible. Walls at or below player level still fade via
            // the camera-line rule above; we don't double-fade.
            let is_structural = match building {
                Some(b) => registry
                    .get(b.item)
                    .map(|d| d.tags.contains(ItemTags::STRUCTURAL))
                    .unwrap_or(false),
                None => false,
            };
            let pos = gt.translation();
            let dx = pos.x - player_pos.x;
            let dz = pos.z - player_pos.z;
            let dy = pos.y - player_pos.y;
            let xz_dist_sq = dx * dx + dz * dz;
            let indoor_ceiling_occluding = indoor
                && is_structural
                && dy > INDOOR_CEILING_MIN_OFFSET
                && xz_dist_sq < INDOOR_REVEAL_RADIUS * INDOOR_REVEAL_RADIUS;

            let occluding = camera_line_occluding || indoor_ceiling_occluding;
            let target = if occluding { 1.0 } else { 0.0 };
            // Indoor reveal goes deeper than the camera-line fade so the
            // floor reads clearly. Foliage stays on its own deeper alpha
            // because four-layer canopies need it to feel see-through.
            let alpha = match kind {
                Some(PropKind::Tree | PropKind::PineTree | PropKind::Bush) => {
                    OCCLUDE_ALPHA_FOLIAGE
                }
                _ if indoor_ceiling_occluding && !camera_line_occluding => {
                    indoor_settings.alpha
                }
                _ if indoor && is_structural => indoor_settings.alpha,
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
        // Refresh `full_alpha` from the latest target — the player can
        // tweak `IndoorRevealSettings.alpha` mid-fade and we want the live
        // value, not whatever was in effect when the fade started.
        state.full_alpha = full_alpha;
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
