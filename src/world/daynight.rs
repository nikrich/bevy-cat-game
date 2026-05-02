use bevy::prelude::*;
use std::f32::consts::PI;

/// Tracks the world's time of day (0.0 to 24.0).
#[derive(Resource)]
pub struct WorldTime {
    /// Current time of day in hours (0.0 - 24.0)
    pub time_of_day: f32,
    /// How many in-game minutes pass per real second
    pub speed: f32,
}

impl Default for WorldTime {
    fn default() -> Self {
        Self {
            time_of_day: 8.0, // Start at morning
            speed: 1.0,       // 1 real minute = 1 in-game hour -> full day in 24 minutes
        }
    }
}

pub fn advance_time(time: Res<Time>, mut world_time: ResMut<WorldTime>) {
    let hours_per_second = world_time.speed / 60.0;
    world_time.time_of_day += hours_per_second * time.delta_secs();
    if world_time.time_of_day >= 24.0 {
        world_time.time_of_day -= 24.0;
    }
}

pub fn update_sun(
    world_time: Res<WorldTime>,
    mut sun_query: Query<(&mut DirectionalLight, &mut Transform)>,
) -> Result {
    let (mut light, mut transform) = sun_query.single_mut()?;

    let t = world_time.time_of_day;

    // Sun angle: rises at 6, peaks at 12, sets at 18
    // Map 6-18 to 0-PI for the arc
    let sun_progress = ((t - 6.0) / 12.0).clamp(0.0, 1.0);
    let sun_angle = sun_progress * PI;

    // Sun is below horizon at night
    let is_day = (5.5..=18.5).contains(&t);

    if is_day {
        let elevation = sun_angle.sin().max(0.05);
        light.illuminance = 4000.0 + 6000.0 * elevation;

        // Sun color shifts: warm at dawn/dusk, white at noon
        let warmth = 1.0 - elevation;
        light.color = Color::srgb(
            1.0,
            0.85 + 0.15 * elevation,
            0.70 + 0.30 * elevation - 0.1 * warmth,
        );

        *transform = Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -sun_angle,
            PI * 0.25,
            0.0,
        ));
    } else {
        // Moonlight -- dim, cool
        light.illuminance = 800.0;
        light.color = Color::srgb(0.7, 0.75, 0.9);

        let night_angle = ((t - 18.5).rem_euclid(24.0) / 11.0) * PI;
        *transform = Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -night_angle.max(0.3),
            PI * 0.75,
            0.0,
        ));
    }

    Ok(())
}

pub fn update_sky_color(world_time: Res<WorldTime>, mut clear_color: ResMut<ClearColor>) {
    let t = world_time.time_of_day;

    // Sky color transitions through the day
    let sky = if t < 5.0 {
        // Night
        Color::srgb(0.08, 0.08, 0.18)
    } else if t < 7.0 {
        // Dawn
        let f = (t - 5.0) / 2.0;
        lerp_color(
            Color::srgb(0.08, 0.08, 0.18),
            Color::srgb(0.85, 0.60, 0.45),
            f,
        )
    } else if t < 9.0 {
        // Morning
        let f = (t - 7.0) / 2.0;
        lerp_color(
            Color::srgb(0.85, 0.60, 0.45),
            Color::srgb(0.54, 0.70, 0.82),
            f,
        )
    } else if t < 16.0 {
        // Daytime
        Color::srgb(0.54, 0.70, 0.82)
    } else if t < 18.0 {
        // Dusk
        let f = (t - 16.0) / 2.0;
        lerp_color(
            Color::srgb(0.54, 0.70, 0.82),
            Color::srgb(0.80, 0.45, 0.35),
            f,
        )
    } else if t < 20.0 {
        // Twilight
        let f = (t - 18.0) / 2.0;
        lerp_color(
            Color::srgb(0.80, 0.45, 0.35),
            Color::srgb(0.08, 0.08, 0.18),
            f,
        )
    } else {
        // Night
        Color::srgb(0.08, 0.08, 0.18)
    };

    clear_color.0 = sky;
}

pub fn update_ambient_light(world_time: Res<WorldTime>, mut ambient: ResMut<GlobalAmbientLight>) {
    let t = world_time.time_of_day;

    let (intensity, color) = if !(5.0..=20.0).contains(&t) {
        // Night
        (150.0, Color::srgb(0.4, 0.45, 0.65))
    } else if t < 7.0 {
        // Dawn
        let f = (t - 5.0) / 2.0;
        (
            150.0 + 350.0 * f,
            lerp_color(
                Color::srgb(0.4, 0.45, 0.65),
                Color::srgb(0.95, 0.80, 0.65),
                f,
            ),
        )
    } else if t < 9.0 {
        // Morning
        let f = (t - 7.0) / 2.0;
        (
            500.0 + 200.0 * f,
            lerp_color(
                Color::srgb(0.95, 0.80, 0.65),
                Color::srgb(1.0, 0.98, 0.95),
                f,
            ),
        )
    } else if t < 16.0 {
        // Day
        (700.0, Color::srgb(1.0, 0.98, 0.95))
    } else if t < 18.0 {
        // Dusk
        let f = (t - 16.0) / 2.0;
        (
            700.0 - 200.0 * f,
            lerp_color(
                Color::srgb(1.0, 0.98, 0.95),
                Color::srgb(0.90, 0.70, 0.55),
                f,
            ),
        )
    } else {
        // Twilight
        let f = (t - 18.0) / 2.0;
        (
            500.0 - 350.0 * f,
            lerp_color(
                Color::srgb(0.90, 0.70, 0.55),
                Color::srgb(0.4, 0.45, 0.65),
                f,
            ),
        )
    };

    ambient.brightness = intensity;
    ambient.color = color;
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    let t = t.clamp(0.0, 1.0);
    Color::srgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

/// Maps `time_of_day` (hours, 0.0..24.0) to a darkness factor in [0.0, 1.0].
///
/// 0.0 means full daylight (no torch); 1.0 means full night. The twilight and
/// dawn windows linearly ramp the factor so the torch fades in/out instead
/// of popping. Windows mirror `update_sky_color`'s 18-20h twilight and 5-7h
/// dawn lerps so the torch fades in as the sky transitions from red to night.
///
/// Cave/dark-interior contributions will be folded in by ORing (taking max
/// of) this value with a cave-occupancy term in `compute_darkness_factor`.
/// Per DEC-024, no cave code exists yet.
pub fn darkness_factor(t: f32) -> f32 {
    if t < 5.0 || t >= 20.0 {
        1.0
    } else if t < 7.0 {
        1.0 - (t - 5.0) / 2.0
    } else if t < 18.0 {
        0.0
    } else {
        (t - 18.0) / 2.0
    }
}

#[cfg(test)]
mod darkness_tests {
    use super::*;

    #[test]
    fn full_night_at_midnight() {
        assert_eq!(darkness_factor(0.0), 1.0);
        assert_eq!(darkness_factor(2.5), 1.0);
        assert_eq!(darkness_factor(4.999), 1.0);
    }

    #[test]
    fn full_night_after_twenty() {
        assert_eq!(darkness_factor(20.0), 1.0);
        assert_eq!(darkness_factor(22.5), 1.0);
        assert_eq!(darkness_factor(23.999), 1.0);
    }

    #[test]
    fn dawn_ramps_one_to_zero() {
        assert!((darkness_factor(5.0) - 1.0).abs() < 1e-5);
        assert!((darkness_factor(6.0) - 0.5).abs() < 1e-5);
        assert!((darkness_factor(7.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn dusk_ramps_zero_to_one() {
        assert!((darkness_factor(18.0) - 0.0).abs() < 1e-5);
        assert!((darkness_factor(19.0) - 0.5).abs() < 1e-5);
        assert!((darkness_factor(19.5) - 0.75).abs() < 1e-5);
    }

    #[test]
    fn full_day_between_seven_and_eighteen() {
        assert_eq!(darkness_factor(7.001), 0.0);
        assert_eq!(darkness_factor(12.0), 0.0);
        assert_eq!(darkness_factor(17.999), 0.0);
    }
}
