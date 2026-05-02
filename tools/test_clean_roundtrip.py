"""Minimal sanity check: import the Mixamo skin FBX, export immediately as
GLB, no processing. If THIS produces a broken kitten in Bevy, the problem
is the Blender FBX import (or glTF export) and we should look for
Mixamo-specific import flags. If it produces a clean kitten, our build
script's post-processing is the destructive step.
"""

from pathlib import Path

import bpy

REPO_ROOT = Path(__file__).resolve().parent.parent
DOWNLOADS = Path.home() / "Downloads"
SKIN_FBX = DOWNLOADS / "Happy Idle (1).fbx"
OUT_GLB = REPO_ROOT / "assets" / "models" / "kittens_animated" / "kitten_12_clean.glb"


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)

    # Mixamo-recommended import flags. The defaults pick a bone orientation
    # heuristic that often misaligns Mixamo's `mixamorig:*` axes; the manual
    # flags below are what every "Mixamo + Blender + glTF" guide converges on.
    bpy.ops.import_scene.fbx(
        filepath=str(SKIN_FBX),
        automatic_bone_orientation=True,
        ignore_leaf_bones=True,
    )

    OUT_GLB.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(
        filepath=str(OUT_GLB.with_suffix("")),
        export_format="GLB",
        export_animations=True,
    )
    print(f"done -> {OUT_GLB.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
