//! Pure-math placement primitives for the decoration system.
//! No Bevy queries here -- only math on coordinates and attach surfaces.

use std::f32::consts::PI;
use bevy::math::Vec3;

/// Granularity of v1 magnetic snap. 0.1m is fine enough that the grid
/// is invisible at iso zoom but coarse enough that two pieces placed
/// "near each other" line up.
pub const FINE_GRID_STEP: f32 = 0.1;

/// Round a world-space coordinate to the nearest `FINE_GRID_STEP`.
pub fn snap_to_fine_grid(value: f32) -> f32 {
    (value / FINE_GRID_STEP).round() * FINE_GRID_STEP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_zero_to_zero() {
        assert_eq!(snap_to_fine_grid(0.0), 0.0);
    }

    #[test]
    fn rounds_to_nearest_tenth() {
        assert!((snap_to_fine_grid(0.34) - 0.3).abs() < 1e-5);
        assert!((snap_to_fine_grid(0.36) - 0.4).abs() < 1e-5);
    }

    #[test]
    fn rounds_negative_correctly() {
        assert!((snap_to_fine_grid(-0.34) + 0.3).abs() < 1e-5);
    }

    #[test]
    fn already_on_grid_unchanged() {
        assert!((snap_to_fine_grid(1.5) - 1.5).abs() < 1e-5);
    }
}

/// 15-degree rotation step in radians for decoration mode.
pub const ROTATION_STEP_RADIANS: f32 = PI / 12.0;

/// Round `radians` to the nearest multiple of `ROTATION_STEP_RADIANS`.
/// Used by R / Shift+R when Alt is not held.
pub fn quantize_rotation(radians: f32) -> f32 {
    (radians / ROTATION_STEP_RADIANS).round() * ROTATION_STEP_RADIANS
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn zero_unchanged() {
        assert!((quantize_rotation(0.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn fifteen_degrees_unchanged() {
        let fifteen = PI / 12.0;
        assert!((quantize_rotation(fifteen) - fifteen).abs() < 1e-5);
    }

    #[test]
    fn snaps_eighteen_to_fifteen() {
        let eighteen = 18.0_f32.to_radians();
        let fifteen = 15.0_f32.to_radians();
        assert!((quantize_rotation(eighteen) - fifteen).abs() < 1e-4);
    }

    #[test]
    fn snaps_thirty_to_thirty() {
        let thirty = 30.0_f32.to_radians();
        assert!((quantize_rotation(thirty) - thirty).abs() < 1e-4);
    }

    #[test]
    fn negative_quantizes() {
        let minus_fifteen = -15.0_f32.to_radians();
        let minus_eighteen = -18.0_f32.to_radians();
        assert!((quantize_rotation(minus_eighteen) - minus_fifteen).abs() < 1e-4);
    }
}

/// What surface a decoration item is attaching to. Drives Y placement
/// and (for walls) facing rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachSurface {
    /// Hit terrain or a non-PlacedItem entity. Y comes from terrain sample.
    Terrain { xz: Vec3 },
    /// Hit a floor's top face. Y is the floor's top.
    FloorTop { xz: Vec3, top_y: f32 },
    /// Hit a wall's side face. Item's back faces normal; Y is mid-wall.
    WallFace { point: Vec3, normal: Vec3 },
    /// Hit non-floor placed-item top face (table top, chest top).
    FurnitureTop { xz: Vec3, top_y: f32 },
}

/// Hit input shape -- decoupled from CursorHit so this stays pure.
#[derive(Clone, Copy, Debug)]
pub struct AttachInput {
    pub point: Vec3,
    pub normal: Vec3,
    pub kind: AttachInputKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachInputKind {
    Terrain,
    Floor { top_y: f32 },
    OtherPlaced { top_y: f32 },
}

/// Decide attach surface from a hit. Priority: floor-top -> wall-face ->
/// furniture-top -> terrain. Wall faces are detected by an upward-facing
/// normal close to horizontal (`|normal.y| < 0.3`); furniture tops require
/// a near-vertical-up normal (`normal.y > 0.9`) so a 45-degree slant
/// doesn't accidentally qualify as a top face.
pub fn pick_attach_surface(input: AttachInput) -> AttachSurface {
    let xz = Vec3::new(input.point.x, 0.0, input.point.z);
    match input.kind {
        AttachInputKind::Terrain => AttachSurface::Terrain { xz },
        AttachInputKind::Floor { top_y } => AttachSurface::FloorTop { xz, top_y },
        AttachInputKind::OtherPlaced { top_y } => {
            if input.normal.y.abs() < 0.3 {
                AttachSurface::WallFace { point: input.point, normal: input.normal }
            } else if input.normal.y > 0.9 {
                AttachSurface::FurnitureTop { xz, top_y }
            } else {
                AttachSurface::Terrain { xz }
            }
        }
    }
}

#[cfg(test)]
mod attach_tests {
    use super::*;
    use bevy::math::Vec3;

    fn input(point: Vec3, normal: Vec3, kind: AttachInputKind) -> AttachInput {
        AttachInput { point, normal, kind }
    }

    #[test]
    fn terrain_routes_to_terrain() {
        let r = pick_attach_surface(input(Vec3::new(1.5, 0.3, 2.5), Vec3::Y, AttachInputKind::Terrain));
        match r {
            AttachSurface::Terrain { xz } => {
                assert_eq!(xz, Vec3::new(1.5, 0.0, 2.5));
            }
            _ => panic!("expected Terrain, got {:?}", r),
        }
    }

    #[test]
    fn floor_top_routes_to_floor_top() {
        let r = pick_attach_surface(input(Vec3::new(0.5, 0.06, 0.5), Vec3::Y, AttachInputKind::Floor { top_y: 0.12 }));
        match r {
            AttachSurface::FloorTop { top_y, .. } => assert_eq!(top_y, 0.12),
            _ => panic!("expected FloorTop, got {:?}", r),
        }
    }

    #[test]
    fn wall_face_normal_routes_to_wall_face() {
        let r = pick_attach_surface(input(
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            AttachInputKind::OtherPlaced { top_y: 1.0 },
        ));
        assert!(matches!(r, AttachSurface::WallFace { .. }));
    }

    #[test]
    fn furniture_top_routes_to_furniture_top() {
        let r = pick_attach_surface(input(
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            AttachInputKind::OtherPlaced { top_y: 0.5 },
        ));
        assert!(matches!(r, AttachSurface::FurnitureTop { .. }));
    }

    #[test]
    fn slanted_normal_falls_back_to_terrain() {
        let r = pick_attach_surface(input(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.5, 0.5, 0.0).normalize(),
            AttachInputKind::OtherPlaced { top_y: 1.0 },
        ));
        assert!(matches!(r, AttachSurface::Terrain { .. }));
    }
}
