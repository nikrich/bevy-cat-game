//! Player input via `leafwing-input-manager` (W0.5 / DEC-013, supersedes
//! DEC-007). The previous hand-rolled `GameInput` resource is gone; every
//! game system now reads `Res<ActionState<Action>>` for keys/buttons and
//! `Res<CursorState>` for cursor-derived state (world position under cursor,
//! whether the pointer is over UI, whether the most recent input came from a
//! gamepad). The iso-rotated movement vector is exposed via the
//! [`iso_movement`] helper so consumers don't repeat the rotation maths.
//!
//! Bindings are intentionally permissive: most actions accept multiple
//! inputs (KB+M *and* gamepad) so the spec's controller-first parity goal
//! (spec §4.4/§4.5) is satisfied without per-system branching.
//!
//! `Place` and `Interact` both bind to `MouseButton::Left`. The UI-gating
//! pass in [`update_cursor_state`] suppresses world-bound left-clicks when
//! the pointer is over UI or the crafting menu is open, so menu clicks
//! never fall through to gather/place.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;

use crate::camera::GameCamera;
use crate::crafting::CraftingState;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<Action>::default())
            .init_resource::<ActionState<Action>>()
            .insert_resource(Action::default_input_map())
            .init_resource::<CursorState>()
            .add_systems(
                PreUpdate,
                (compute_cursor_world, update_cursor_state)
                    .chain()
                    .after(InputManagerSystem::Update),
            );
    }
}

/// Every action the game cares about. Exhaustive per spec §4.4/§4.5 plus
/// game-specific verbs (Nap/Examine/Mark/ToggleCraft/Save) and per-slot
/// hotbar bindings. Crouch/ZoomIn/ZoomOut are declared so the binding map
/// is complete; their consumers ship in later phases.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum Action {
    #[actionlike(DualAxis)]
    Move,
    Jump,
    Sprint,
    Crouch,
    Interact,
    Place,
    RotatePiece,
    ToggleBuild,
    ToggleEditTerrain,
    ToggleInventory,
    ToggleCraft,
    Save,
    Nap,
    Examine,
    Mark,
    MenuUp,
    MenuDown,
    MenuConfirm,
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
    HotbarNext,
    HotbarPrev,
    ZoomIn,
    ZoomOut,
}

impl Action {
    pub fn default_input_map() -> InputMap<Self> {
        let mut map = InputMap::default();

        // Movement: WASD + arrow keys, gamepad left stick.
        map.insert_dual_axis(Self::Move, VirtualDPad::wasd());
        map.insert_dual_axis(Self::Move, VirtualDPad::arrow_keys());
        map.insert_dual_axis(Self::Move, GamepadStick::LEFT);

        // Core verbs. Both keyboard and gamepad bindings are present so
        // controller-first parity holds out of the box.
        map.insert(Self::Jump, KeyCode::Space);
        map.insert(Self::Jump, GamepadButton::South);

        map.insert(Self::Sprint, KeyCode::ShiftLeft);
        map.insert(Self::Sprint, KeyCode::ShiftRight);
        map.insert(Self::Sprint, GamepadButton::LeftThumb);

        map.insert(Self::Crouch, KeyCode::ControlLeft);
        map.insert(Self::Crouch, GamepadButton::RightThumb);

        // Mouse-left is intentionally *not* bound to Interact/Place: mouse
        // clicks go through `CursorState::world_click` so the UI focus pass
        // can suppress them when the pointer is over UI. Keyboard/gamepad
        // bindings here remain unconditional.
        map.insert(Self::Interact, KeyCode::KeyE);
        map.insert(Self::Interact, GamepadButton::South);

        map.insert(Self::Place, KeyCode::Space);
        map.insert(Self::Place, GamepadButton::South);

        map.insert(Self::RotatePiece, KeyCode::KeyR);
        map.insert(Self::RotatePiece, GamepadButton::East);

        map.insert(Self::ToggleBuild, KeyCode::KeyB);
        map.insert(Self::ToggleBuild, GamepadButton::North);

        map.insert(Self::ToggleEditTerrain, KeyCode::KeyT);

        map.insert(Self::ToggleInventory, KeyCode::KeyI);
        map.insert(Self::ToggleInventory, GamepadButton::Select);

        map.insert(Self::ToggleCraft, KeyCode::Tab);
        map.insert(Self::ToggleCraft, GamepadButton::West);

        map.insert(Self::Save, KeyCode::F5);
        map.insert(Self::Save, GamepadButton::Start);

        // Cat verbs (Phase B).
        map.insert(Self::Nap, KeyCode::KeyZ);
        map.insert(Self::Examine, KeyCode::KeyX);
        map.insert(Self::Mark, KeyCode::KeyC);

        // Menu navigation. Inputs deliberately overlap with movement keys
        // since menu nav and gameplay never run in the same frame; the
        // consumer (UI / pause menu) gates by state.
        map.insert(Self::MenuUp, KeyCode::KeyW);
        map.insert(Self::MenuUp, KeyCode::ArrowUp);
        map.insert(Self::MenuUp, GamepadButton::DPadUp);
        map.insert(Self::MenuDown, KeyCode::KeyS);
        map.insert(Self::MenuDown, KeyCode::ArrowDown);
        map.insert(Self::MenuDown, GamepadButton::DPadDown);
        map.insert(Self::MenuConfirm, KeyCode::Enter);
        map.insert(Self::MenuConfirm, KeyCode::KeyE);
        map.insert(Self::MenuConfirm, GamepadButton::South);

        // Hotbar slots.
        let hotbar_keys = [
            (Self::Hotbar1, KeyCode::Digit1),
            (Self::Hotbar2, KeyCode::Digit2),
            (Self::Hotbar3, KeyCode::Digit3),
            (Self::Hotbar4, KeyCode::Digit4),
            (Self::Hotbar5, KeyCode::Digit5),
            (Self::Hotbar6, KeyCode::Digit6),
            (Self::Hotbar7, KeyCode::Digit7),
            (Self::Hotbar8, KeyCode::Digit8),
            (Self::Hotbar9, KeyCode::Digit9),
        ];
        for (action, key) in hotbar_keys {
            map.insert(action, key);
        }
        map.insert(Self::HotbarNext, GamepadButton::RightTrigger);
        map.insert(Self::HotbarPrev, GamepadButton::LeftTrigger);
        // Keyboard cycle so the player can reach placeables past slot 9
        // (Wall variants live at slots 12-15) without using the inventory UI.
        map.insert(Self::HotbarNext, KeyCode::KeyE);
        map.insert(Self::HotbarPrev, KeyCode::KeyQ);
        map.insert(Self::HotbarNext, MouseScrollDirection::DOWN);
        map.insert(Self::HotbarPrev, MouseScrollDirection::UP);

        // Camera zoom — declared for binding completeness; consumer lands
        // when the camera grows zoom controls.
        map.insert(Self::ZoomIn, KeyCode::Equal);
        map.insert(Self::ZoomOut, KeyCode::Minus);

        map
    }
}

/// Cursor-derived state that leafwing doesn't track on its own. This is
/// where `compute_cursor_world` and the UI focus pass deposit their results
/// so consumers can ask one resource instead of recomputing per frame.
#[derive(Resource, Default)]
pub struct CursorState {
    /// World position under the mouse cursor, raycast against the Y=0 plane.
    /// Used for terrain-relative XZ snapping where the actual surface
    /// elevation doesn't matter (gathering distance, basic placement grid).
    pub cursor_world: Option<Vec3>,
    /// Rapier raycast hit against actual world geometry (terrain trimesh +
    /// placed building colliders + player). This is the iso-correct "what
    /// pixel are you visually pointing at" — the build system uses it to
    /// stack pieces on tops of walls / tables instead of guessing from the
    /// flat-ground projection.
    pub cursor_hit: Option<CursorHit>,
    /// True when the pointer is over an interactive UI node this frame.
    pub pointer_over_ui: bool,
    /// True if the most recent meaningful input came from a gamepad.
    pub using_gamepad: bool,
    /// True only if a left-mouse press this frame should be treated as a
    /// world click. False when the pointer is over UI or the crafting menu
    /// is open.
    pub world_click: bool,
}

/// Geometry-aware hit returned by `compute_cursor_world`'s rapier raycast.
/// `entity` is the collider entity (terrain chunk, placed building, player)
/// — consumers check what kind of entity it is via component queries.
#[derive(Clone, Copy, Debug)]
pub struct CursorHit {
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Iso-rotated movement vector. Convert leafwing's raw `Move` axis pair into
/// the vector all the world-space gameplay systems were tuned for.
pub fn iso_movement(action_state: &ActionState<Action>) -> Vec2 {
    let raw = action_state.clamped_axis_pair(&Action::Move);
    if raw.length_squared() < 0.0001 {
        return Vec2::ZERO;
    }
    let dir = raw.normalize();
    let angle = std::f32::consts::FRAC_PI_4;
    Vec2::new(
        dir.x * angle.cos() - dir.y * angle.sin(),
        dir.x * angle.sin() + dir.y * angle.cos(),
    )
}

/// Iso-axis movement before the camera rotation, useful for UI nav.
pub fn raw_movement(action_state: &ActionState<Action>) -> Vec2 {
    let raw = action_state.clamped_axis_pair(&Action::Move);
    if raw.length_squared() < 0.0001 {
        Vec2::ZERO
    } else {
        raw.normalize()
    }
}

fn compute_cursor_world(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    rapier: ReadRapierContext,
    mut cursor: ResMut<CursorState>,
) {
    cursor.cursor_world = None;
    cursor.cursor_hit = None;
    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_gt)) = camera_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
    let Ok(ray) = camera.viewport_to_world(camera_gt, cursor_pos) else { return };

    // Y=0 ground plane — kept for legacy consumers and as a fallback when
    // the rapier raycast misses everything.
    let denom = ray.direction.y;
    if denom.abs() >= 0.001 {
        let t = -ray.origin.y / denom;
        if t >= 0.0 {
            cursor.cursor_world = Some(ray.origin + *ray.direction * t);
        }
    }

    // Rapier raycast: returns the closest collider the camera ray hits.
    // `solid=true` so rays starting inside a collider report time_of_impact=0.
    if let Ok(ctx) = rapier.single() {
        if let Some((entity, hit)) =
            ctx.cast_ray_and_get_normal(ray.origin, *ray.direction, 1000.0, true, QueryFilter::default())
        {
            cursor.cursor_hit = Some(CursorHit {
                entity,
                point: hit.point,
                normal: hit.normal,
            });
        }
    }
}

fn update_cursor_state(
    interactions: Query<&Interaction>,
    crafting: Res<CraftingState>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut cursor: ResMut<CursorState>,
) {
    cursor.pointer_over_ui = interactions
        .iter()
        .any(|i| !matches!(i, Interaction::None));

    // Mouse-left isn't bound to a leafwing action; we read it raw here so
    // the UI focus pass can gate world clicks against UI/crafting state in
    // one central place. Consumers read `cursor.world_click` instead of
    // `Res<ButtonInput<MouseButton>>` to inherit the gating.
    cursor.world_click = mouse.just_pressed(MouseButton::Left)
        && !cursor.pointer_over_ui
        && !crafting.open;

    // Cheap "is gamepad active" probe: any non-zero stick deflection or any
    // pressed button on any connected gamepad sets the flag for the frame.
    let mut gamepad_active = false;
    for gamepad in &gamepads {
        let lx = gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        let ly = gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
        if lx.abs() > 0.15 || ly.abs() > 0.15 {
            gamepad_active = true;
            break;
        }
    }
    if gamepad_active {
        cursor.using_gamepad = true;
    }
}
