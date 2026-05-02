"""Strip Rigify rig from each kitten GLB and export Mixamo-ready FBX.

Mixamo's auto-rigger wants a single un-rigged mesh in T-pose. The kittens
ship with a Rigify (Blender meta-rig) skeleton that's bipedal and unanimated,
which means Mixamo can't use it directly — its expected bones are
`mixamorig:LeftHand` etc., not `hand_tweak.L`. So we keep the bind-pose mesh
and throw the skeleton away; Mixamo re-rigs each upload with its standard
humanoid bones, and because that naming is identical across uploads, a single
animation set downloaded for any one kitten will play on all twelve.

Output: ../mixamo_prep/kitten_NN.fbx for each 1..12.glb under
assets/models/kittens/glb/.

Usage:
    /Applications/Blender.app/Contents/MacOS/Blender \\
        --background --python tools/prep_kittens_for_mixamo.py
"""

from pathlib import Path

import bpy

REPO_ROOT = Path(__file__).resolve().parent.parent
GLB_DIR = REPO_ROOT / "assets" / "models" / "kittens" / "glb"
OUT_DIR = REPO_ROOT / "mixamo_prep"


def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def prep_one(glb_path: Path, fbx_path: Path) -> None:
    reset_scene()
    bpy.ops.import_scene.gltf(filepath=str(glb_path))

    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    armatures = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]

    if not meshes:
        print(f"  ! no meshes in {glb_path.name}, skipping")
        return

    # Drop the armature deform so the mesh stays at its bind-pose vertex
    # positions (Rigify's rest pose is T-pose), and clear vertex groups so
    # Mixamo's auto-rigger doesn't try to honour the old skinning weights.
    for mesh in meshes:
        for mod in list(mesh.modifiers):
            if mod.type == "ARMATURE":
                mesh.modifiers.remove(mod)
        mesh.vertex_groups.clear()

    # Unparent meshes from the armature, preserving their world transforms,
    # so deleting the armature next doesn't drag them with it.
    bpy.ops.object.select_all(action="DESELECT")
    for mesh in meshes:
        mesh.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")

    bpy.ops.object.select_all(action="DESELECT")
    for armature in armatures:
        armature.select_set(True)
    if armatures:
        bpy.ops.object.delete()

    # Mixamo's auto-rigger expects a single mesh object — join sub-meshes
    # (e.g. a separate frog hat) into the body so they share one upload.
    bpy.ops.object.select_all(action="DESELECT")
    for mesh in meshes:
        mesh.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()

    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    bpy.ops.export_scene.fbx(
        filepath=str(fbx_path),
        use_selection=True,
        object_types={"MESH"},
        apply_unit_scale=True,
        bake_space_transform=True,
        path_mode="COPY",
        embed_textures=True,
        add_leaf_bones=False,
    )


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for i in range(1, 13):
        glb = GLB_DIR / f"{i}.glb"
        fbx = OUT_DIR / f"kitten_{i:02d}.fbx"
        if not glb.exists():
            print(f"missing: {glb}")
            continue
        print(f"=== {glb.name} -> {fbx.relative_to(REPO_ROOT)} ===")
        prep_one(glb, fbx)
    print(f"done -> {OUT_DIR.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
