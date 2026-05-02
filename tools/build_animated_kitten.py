"""Combine a skinned Mixamo FBX (Happy Idle, with mesh) and any number of
animation-only FBX files (Happy Walk, etc.) into one GLB with all clips.

Mixamo emits one FBX per animation, each carrying its own copy of the
auto-rigged Mixamo skeleton. Bevy wants a single glTF with a mesh and a
list of named animation clips, so we:

  1. Import the skinned FBX — this owns the mesh, the skeleton we keep, and
     its bundled action.
  2. For each animation-only FBX, import it (which spawns a duplicate
     armature), steal its action onto the kept armature's NLA stack, and
     delete the duplicate.
  3. Export GLB with `NLA_TRACKS` so each NLA strip becomes a named
     glTF animation, addressable from Bevy by name.

Usage:
    /Applications/Blender.app/Contents/MacOS/Blender \\
        --background --python tools/build_animated_kitten.py
"""

from pathlib import Path

import bpy

REPO_ROOT = Path(__file__).resolve().parent.parent
DOWNLOADS = Path.home() / "Downloads"

# Each entry is (FBX path, clip name to expose to Bevy). The skin FBX is
# the one whose mesh we keep; subsequent FBX imports may also carry a mesh
# (Mixamo lets you pick With/Without Skin per download), but we discard any
# extra meshes after extracting their action so the export has just one
# kitten body.
SKIN_FBX = (DOWNLOADS / "Happy Idle (1).fbx", "Idle")
ANIM_FBXS = [
    (DOWNLOADS / "Happy Walk (1).fbx", "Walk"),
    (DOWNLOADS / "Running.fbx", "Run"),
    (DOWNLOADS / "Jumping.fbx", "Jump"),
    (DOWNLOADS / "Sneaking Forward.fbx", "Sneak"),
]
# Mixamo's auto-rigger hands back an untextured mesh — its preview never
# shows the upload's texture and the FBX it emits drops the embedded image.
# We restore the kitten's diffuse from the original asset texture so the
# Bevy export carries colour.
TEXTURE_PATH = REPO_ROOT / "assets" / "models" / "kittens" / "glb" / "low_poly.png"
OUT_GLB = REPO_ROOT / "assets" / "models" / "kittens_animated" / "kitten_12.glb"


def clean_skin_weights(mesh_objects) -> None:
    """DO NOT CALL — kept for reference. This step was the root cause of the
    Phase-5 skinning corruption: pre-empting Blender's glTF exporter with our
    own `vertex_group_limit_total(limit=4)` selected a different "best 4"
    bones than the exporter's native influence pruning, producing skin
    matrices whose determinants flipped during pose evaluation. Bind pose
    rendered fine; animation playback corrupted normals (face hidden, tail
    through chest). Letting the glTF exporter handle 4-bone limiting itself
    yields working output.
    """
    for mesh in mesh_objects:
        bpy.ops.object.select_all(action="DESELECT")
        bpy.context.view_layer.objects.active = mesh
        mesh.select_set(True)
        bpy.ops.object.vertex_group_clean(group_select_mode="ALL", limit=0.0)
        bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
        bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL")


def fix_normals(mesh_objects) -> None:
    """Recalculate face normals to point outward.

    Mixamo's FBX export often arrives with inconsistent face winding
    (some islands inverted), which after the FBX -> glTF round-trip shades
    surfaces as if lit from inside. `normals_make_consistent(inside=False)`
    is Blender's "Recalculate Outside" — it flood-fills outward-facing
    normals across each connected island.
    """
    for mesh in mesh_objects:
        bpy.ops.object.select_all(action="DESELECT")
        bpy.context.view_layer.objects.active = mesh
        mesh.select_set(True)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.normals_make_consistent(inside=False)
        bpy.ops.object.mode_set(mode="OBJECT")


def reapply_texture(mesh_objects, texture_path: Path) -> None:
    """Point each material's existing Image Texture node at our PNG.

    Mixamo's FBX ships materials with the right node graph already wired up
    — Principled BSDF, image texture node, links to base colour — but the
    image slot is empty because the upload's embedded PNG was stripped.
    We *only* fill the image slot, never rebuild nodes, so we don't clobber
    roughness/metallic/normal links that the auto-rigger authored.
    """
    if not texture_path.exists():
        print(f"  ! texture {texture_path} missing, skipping retexture")
        return
    image = bpy.data.images.load(str(texture_path), check_existing=True)
    image.pack()

    for mesh in mesh_objects:
        for slot in mesh.material_slots:
            mat = slot.material
            if mat is None or not mat.use_nodes:
                continue
            tex = next((n for n in mat.node_tree.nodes if n.type == "TEX_IMAGE"), None)
            if tex is None:
                # Fallback: no image-texture node existed. Create one and
                # connect to the existing BSDF without disturbing other
                # links on that BSDF.
                bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
                if bsdf is None:
                    continue
                tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
                mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
            tex.image = image


def push_to_nla(armature, action, clip_name):
    """Park `action` as an NLA strip on `armature` under `clip_name`.

    The track name is what Blender's glTF exporter writes as the animation
    name in NLA_TRACKS mode, so this is the string Bevy will look up by.
    """
    action.name = clip_name
    if not armature.animation_data:
        armature.animation_data_create()
    track = armature.animation_data.nla_tracks.new()
    track.name = clip_name
    start = int(action.frame_range[0])
    strip = track.strips.new(clip_name, start, action)
    strip.name = clip_name
    # Clear the active action so the NLA stack drives playback (and so the
    # next imported FBX has a clean slate to attach its own action to).
    armature.animation_data.action = None


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)

    skin_path, skin_clip = SKIN_FBX
    bpy.ops.import_scene.fbx(filepath=str(skin_path))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not meshes:
        raise RuntimeError(
            f"{skin_path.name} has no mesh — re-download from Mixamo with the "
            f"Skin dropdown set to 'With Skin', not 'Without Skin'."
        )
    armatures = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]
    if not armatures:
        raise RuntimeError(f"no armature found in {skin_path}")
    base = armatures[0]

    reapply_texture(meshes, TEXTURE_PATH)
    # NB: clean_skin_weights() and fix_normals() are intentionally NOT called.
    # clean_skin_weights' limit-to-4 fought the glTF exporter's own influence
    # pruning and was the root cause of the Phase-5 corruption (see its
    # docstring). fix_normals' `normals_make_consistent` picks an arbitrary
    # "outside" per island on the kitten's ~150 disconnected meshes and gets
    # roughly half wrong. Both functions stay defined as documentation of
    # what NOT to do.

    if base.animation_data and base.animation_data.action:
        push_to_nla(base, base.animation_data.action, skin_clip)

    for anim_fbx, clip_name in ANIM_FBXS:
        if not anim_fbx.exists():
            print(f"  ! missing {anim_fbx}, skipping {clip_name}")
            continue
        existing_objects = {o.name for o in bpy.context.scene.objects}
        existing_actions = set(bpy.data.actions.keys())

        bpy.ops.import_scene.fbx(filepath=str(anim_fbx))

        new_objects = [o for o in bpy.context.scene.objects if o.name not in existing_objects]
        new_action_names = set(bpy.data.actions.keys()) - existing_actions
        if not new_action_names:
            print(f"  ! no new action from {anim_fbx.name}, skipping")
            for obj in new_objects:
                bpy.data.objects.remove(obj, do_unlink=True)
            continue

        new_action = bpy.data.actions[next(iter(new_action_names))]
        push_to_nla(base, new_action, clip_name)

        # Drop everything the FBX brought in — armature *and* any duplicate
        # mesh ("With Skin" downloads each ship a copy of the body). We only
        # needed the action, which now lives on the base armature's NLA.
        for obj in new_objects:
            bpy.data.objects.remove(obj, do_unlink=True)

    OUT_GLB.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(
        filepath=str(OUT_GLB.with_suffix("")),  # exporter appends .glb itself
        export_format="GLB",
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_apply=True,
    )
    print(f"done -> {OUT_GLB.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
