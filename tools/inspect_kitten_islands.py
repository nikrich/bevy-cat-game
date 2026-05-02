"""Count disconnected mesh islands per kitten GLB and their bounding boxes.

If a kitten's mesh has multiple far-apart islands, hidden hat variants are
baked into the asset and the Mixamo auto-rig was correctly rigging *all*
of them — explaining the stacked-hats artifact in Bevy.
"""

from pathlib import Path

import bpy
from mathutils import Vector

REPO_ROOT = Path(__file__).resolve().parent.parent
GLB_DIR = REPO_ROOT / "assets" / "models" / "kittens" / "glb"


def islands_for(glb_path: Path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(glb_path))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not meshes:
        return []
    mesh = meshes[0]
    bpy.context.view_layer.objects.active = mesh
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="DESELECT")
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.mode_set(mode="EDIT")
    # Separate by loose parts to count islands
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.separate(type="LOOSE")
    bpy.ops.object.mode_set(mode="OBJECT")
    parts = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    info = []
    for p in parts:
        bbox = [p.matrix_world @ Vector(c) for c in p.bound_box]
        xs, ys, zs = zip(*((v.x, v.y, v.z) for v in bbox))
        info.append({
            "name": p.name,
            "verts": len(p.data.vertices),
            "x": (round(min(xs), 2), round(max(xs), 2)),
            "y": (round(min(ys), 2), round(max(ys), 2)),
            "z": (round(min(zs), 2), round(max(zs), 2)),
        })
    return info


def main():
    for n in [1, 5, 12]:
        path = GLB_DIR / f"{n}.glb"
        info = islands_for(path)
        print(f"\n=== kitten {n} ({len(info)} islands) ===")
        for i in sorted(info, key=lambda d: -d["verts"]):
            print(f"  {i['name']:25} verts={i['verts']:>6}  x={i['x']}  y={i['y']}  z={i['z']}")


if __name__ == "__main__":
    main()
