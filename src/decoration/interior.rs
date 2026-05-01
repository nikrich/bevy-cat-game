use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use crate::items::{AabbBounds, ItemDef};
use crate::building::placement::cube_target_width;

/// Marker for an entity whose interior mesh+material are pending GLB
/// load. `resolve_interior_spawns` polls these and once the named node
/// resolves, spawns Mesh3d+MeshMaterial3d children for each primitive.
/// Removed from the entity once resolved.
///
/// `child_offset` and `child_scale_mul` are pre-computed at spawn time
/// (when we already have the catalog AABB) so resolve doesn't need
/// registry / catalog access. They make every interior asset render with
/// its AABB centred at the parent transform -- without that, GLB nodes
/// whose origin is at a corner / floor placed visibly off-grid even with
/// correct snap.
#[derive(Component)]
pub struct InteriorSpawnRequest {
    pub gltf: Handle<Gltf>,
    pub node_name: String,
    /// Local translation applied to each spawned child mesh. Cancels the
    /// asset's intrinsic origin offset (set to `-aabb.centre`).
    pub child_offset: Vec3,
    /// Per-axis scale multiplier applied on top of the GLB node's own
    /// scale. `Vec3::ONE` for most assets; doors set `x` so the world
    /// width = 1 cube cell.
    pub child_scale_mul: Vec3,
}

/// Compute the per-asset child offset (cancels intrinsic origin offset so
/// the AABB centres at the parent transform) and per-axis scale multiplier
/// (door / window categories get x stretched to fit a fixed cube width).
/// Returns `(child_offset, child_scale_mul, effective_aabb)` where the
/// effective AABB is the post-stretch AABB used for footprint and Y
/// placement.
pub(crate) fn interior_render_params(
    def: &ItemDef,
    aabb: AabbBounds,
) -> (Vec3, Vec3, AabbBounds) {
    let parent_scale = def.form.placement_scale();
    let mut scale_mul = Vec3::ONE;
    let mut effective = aabb;
    if let Some(target_world_x) = cube_target_width(def) {
        if aabb.size().x > 1e-4 {
            // World X = parent_scale * scale_mul.x * aabb.size().x -> target
            let target = target_world_x / (parent_scale * aabb.size().x);
            scale_mul.x = target;
            effective = AabbBounds {
                min: Vec3::new(aabb.min.x * target, aabb.min.y, aabb.min.z),
                max: Vec3::new(aabb.max.x * target, aabb.max.y, aabb.max.z),
            };
        }
    }
    // Recentre: the child meshes already get scaled by node TRS + scale_mul,
    // so we shift them by -effective.centre() to land the AABB centre at
    // the parent's local origin.
    let child_offset = -effective.center();
    (child_offset, scale_mul, effective)
}

/// Async resolver for `InteriorSpawnRequest` -- once the parent GLB is
/// loaded and the named node's mesh asset is ready, spawn one
/// Mesh3d+MeshMaterial3d child per primitive on the placed entity, then
/// remove the request component. The node's local transform is ignored;
/// the placed entity already carries the world transform from placement.
pub(crate) fn resolve_interior_spawns(
    mut commands: Commands,
    requests: Query<(Entity, &InteriorSpawnRequest)>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    for (entity, req) in &requests {
        let Some(gltf) = gltfs.get(&req.gltf) else { continue };
        let Some(node_handle) = gltf.named_nodes.get(req.node_name.as_str()) else {
            warn!("[interior] node '{}' not in parent GLB", req.node_name);
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(node) = gltf_nodes.get(node_handle) else { continue };
        let Some(mesh_handle) = node.mesh.as_ref() else {
            // Some nodes are bare transforms with no mesh -- nothing to render.
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else { continue };

        // Preserve the node's rotation + scale so per-item authoring
        // intent (e.g. plant.008 has scale 0.108) carries through.
        // Translation comes from the source scene's grid layout -- we
        // explicitly drop it. `child_offset` re-shifts so the asset's AABB
        // centre lands at the parent transform; `child_scale_mul` adds the
        // door-width stretch on top of the node scale.
        let local_tf = Transform {
            translation: req.child_offset,
            rotation: node.transform.rotation,
            scale: node.transform.scale * req.child_scale_mul,
        };
        commands.entity(entity).with_children(|parent| {
            for prim in &gltf_mesh.primitives {
                let mat = prim
                    .material
                    .clone()
                    .unwrap_or_default();
                parent.spawn((
                    Mesh3d(prim.mesh.clone()),
                    MeshMaterial3d(mat),
                    local_tf,
                ));
            }
        });
        commands.entity(entity).remove::<InteriorSpawnRequest>();
    }
}
