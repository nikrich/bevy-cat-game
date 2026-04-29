use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::GameCamera;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameInput>()
            .add_systems(
                PreUpdate,
                (
                    clear_input,
                    read_keyboard_mouse,
                    read_gamepad,
                    compute_cursor_world,
                )
                    .chain(),
            )
            .add_systems(
                PreUpdate,
                resolve_world_click
                    .after(compute_cursor_world)
                    .after(bevy::ui::UiSystems::Focus),
            );
    }
}

/// Unified input state read by all game systems.
#[derive(Resource, Default)]
pub struct GameInput {
    /// Movement direction (normalized, already rotated for isometric camera)
    pub movement: Vec2,
    /// Raw movement before iso rotation (for UI navigation)
    pub raw_movement: Vec2,

    // Actions -- true on the frame they were triggered
    pub interact: bool,
    pub toggle_craft: bool,
    pub toggle_build: bool,
    pub place: bool,
    pub rotate: bool,
    pub save: bool,
    pub menu_up: bool,
    pub menu_down: bool,
    pub menu_confirm: bool,

    /// Build slot selection (1-5), None if not pressed
    pub build_select: Option<usize>,

    // Phase B cat verbs.
    /// Z held: cat curls up to nap (banked after a hold-window).
    pub nap_held: bool,
    /// X tapped: cat examines the nearest notable thing.
    pub examine: bool,
    /// Shift held: cat moves at stalking speed (lower stance, slower).
    pub stalk_held: bool,
    /// C held: cat marks this cell as their own (idempotent, banked after a hold).
    pub mark_held: bool,

    /// World position under mouse cursor (for placement)
    pub cursor_world: Option<Vec3>,

    /// Whether input is coming from gamepad (affects UI hints)
    pub using_gamepad: bool,

    /// True if cursor is hovering or pressing any interactive UI node this frame.
    /// Set after UI focus has been resolved so consumers can disambiguate UI clicks from world clicks.
    pub pointer_over_ui: bool,

    /// Raw left-mouse just-pressed state. Use this when you want UI clicks too;
    /// otherwise read `interact` / `place` which are gated by `pointer_over_ui`.
    pub mouse_left_just_pressed: bool,
}

fn clear_input(mut input: ResMut<GameInput>) {
    *input = GameInput::default();
}

fn read_keyboard_mouse(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut input: ResMut<GameInput>,
) {
    // Movement
    let mut dir = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    if dir.length_squared() > 0.0 {
        dir = dir.normalize();
    }
    input.raw_movement = dir;

    // Rotate for isometric camera
    let angle = std::f32::consts::FRAC_PI_4;
    input.movement = Vec2::new(
        dir.x * angle.cos() - dir.y * angle.sin(),
        dir.x * angle.sin() + dir.y * angle.cos(),
    );

    // Actions -- mouse-derived contributions to interact/place are deferred to
    // `resolve_world_click` so UI clicks don't fall through to the world.
    input.mouse_left_just_pressed = mouse.just_pressed(MouseButton::Left);
    input.interact = keyboard.just_pressed(KeyCode::KeyE);
    input.toggle_craft = keyboard.just_pressed(KeyCode::Tab);
    input.toggle_build = keyboard.just_pressed(KeyCode::KeyB);
    input.place = keyboard.just_pressed(KeyCode::Space);
    input.rotate = keyboard.just_pressed(KeyCode::KeyR);
    input.save = keyboard.just_pressed(KeyCode::F5);

    // Cat verbs (Phase B).
    input.nap_held = keyboard.pressed(KeyCode::KeyZ);
    input.examine = keyboard.just_pressed(KeyCode::KeyX);
    input.stalk_held =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    input.mark_held = keyboard.pressed(KeyCode::KeyC);

    // Menu navigation (W/S when in menus)
    input.menu_up = keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp);
    input.menu_down = keyboard.just_pressed(KeyCode::KeyS) || keyboard.just_pressed(KeyCode::ArrowDown);
    input.menu_confirm = keyboard.just_pressed(KeyCode::KeyE) || keyboard.just_pressed(KeyCode::Enter);

    // Build slot
    let slot_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];
    for (i, key) in slot_keys.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            input.build_select = Some(i);
        }
    }
}

fn read_gamepad(
    gamepads: Query<&Gamepad>,
    mut input: ResMut<GameInput>,
) {
    let Ok(gamepad) = gamepads.single() else {
        return;
    };

    input.using_gamepad = true;

    // Left stick movement
    let stick_x = gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
    let stick_y = gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);

    let deadzone = 0.15;
    let raw = if stick_x.abs() > deadzone || stick_y.abs() > deadzone {
        let dir = Vec2::new(stick_x, stick_y).normalize();
        input.raw_movement = dir;
        dir
    } else {
        Vec2::ZERO
    };

    if raw.length_squared() > 0.0 {
        let angle = std::f32::consts::FRAC_PI_4;
        input.movement = Vec2::new(
            raw.x * angle.cos() - raw.y * angle.sin(),
            raw.x * angle.sin() + raw.y * angle.cos(),
        );
    }

    // Buttons
    if gamepad.just_pressed(GamepadButton::South) {
        input.interact = true;
        input.menu_confirm = true;
        input.place = true;
    }
    if gamepad.just_pressed(GamepadButton::West) {
        input.toggle_craft = true;
    }
    if gamepad.just_pressed(GamepadButton::North) {
        input.toggle_build = true;
    }
    if gamepad.just_pressed(GamepadButton::East) {
        input.rotate = true;
    }

    // D-pad for menu navigation
    if gamepad.just_pressed(GamepadButton::DPadUp) {
        input.menu_up = true;
    }
    if gamepad.just_pressed(GamepadButton::DPadDown) {
        input.menu_down = true;
    }

    // Bumpers for build slot cycling
    if gamepad.just_pressed(GamepadButton::RightTrigger) {
        input.build_select = Some(99); // signal "next"
    }
    if gamepad.just_pressed(GamepadButton::LeftTrigger) {
        input.build_select = Some(98); // signal "prev"
    }

    if gamepad.just_pressed(GamepadButton::Start) {
        input.save = true;
    }
}

/// Raycast from mouse cursor through camera to find world position on terrain plane.
fn compute_cursor_world(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    mut input: ResMut<GameInput>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_gt)) = camera_query.single() else { return };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Cast ray from camera through cursor
    let Ok(ray) = camera.viewport_to_world(camera_gt, cursor_pos) else {
        return;
    };

    // Intersect with Y=0 plane (approximate terrain level)
    let denom = ray.direction.y;
    if denom.abs() < 0.001 {
        return;
    }

    let t = -ray.origin.y / denom;
    if t < 0.0 {
        return;
    }

    let world_pos = ray.origin + *ray.direction * t;
    input.cursor_world = Some(world_pos);
}

/// Decides whether a left-mouse press counts as a world click (gather/place) or a UI click.
/// Runs after Bevy has updated `Interaction` for UI nodes this frame.
fn resolve_world_click(
    interactions: Query<&Interaction>,
    crafting: Res<crate::crafting::CraftingState>,
    mut input: ResMut<GameInput>,
) {
    let pointer_over_ui = interactions
        .iter()
        .any(|i| !matches!(i, Interaction::None));
    input.pointer_over_ui = pointer_over_ui;

    let world_click = input.mouse_left_just_pressed && !pointer_over_ui && !crafting.open;
    if world_click {
        input.interact = true;
        input.place = true;
    }
}
