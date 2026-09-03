//! Cãmera de voo livre ("freefly") usada para navegar pela cena.

use bevy::app::App;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::{
    ButtonInput, Camera3d, Commands, Component, EulerRot, KeyCode, MessageReader,
    PerspectiveProjection, Plugin, Projection, Quat, Query, Res, Single, Startup, Time, Transform,
    Update, Vec3, default,
};
use bevy::window::{CursorGrabMode, CursorOptions};

/// Marca a câmera de voo livre e guarda o estado de orientação e os parâmetros de movimento.
#[derive(Component)]
pub struct FreeFlyCamera {
    /// Rotação horizontal acumulada, em radianos.
    pub yaw: f32,
    /// Rotação vertical acumulada, em radianos.
    pub pitch: f32,
    /// Velocidade de movimento da câmera, unidades por seg.
    pub speed: f32,
    /// Multiplicador de velocidade apertando shift
    pub sprint_multiplier: f32,
    /// Sensibilidade do mouse, aplicada ao delta de movimento do mouse
    pub sensitivity: f32,
}

impl Default for FreeFlyCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 20.0,
            sprint_multiplier: 10.0,
            sensitivity: 0.02,
        }
    }
}

/// Limite de pitch, em radianos, para não permitir que a câmera vire de cabeça para baixo.
const PITCH_LIMIT: f32 = 1.54;

/// Registra o plugin de câmera de voo livre.
pub struct FreeFlyCameraPlugin;

impl Plugin for FreeFlyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_free_fly_camera)
            .add_systems(Update, (mouse_look, keyboard_movement, cursor_release));
    }
}

/// Spawna a câmera de voo livre com projeção perspectiva e captura o cursor imediatamente.
fn spawn_free_fly_camera(mut commands: Commands, mut cursor_options: Single<&mut CursorOptions>) {
    let translation = Vec3::new(0.0, 600.0, 1200.0);
    let mut transform = Transform::from_translation(translation);
    transform.look_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y);
    let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);

    commands.spawn((
        Camera3d::default(),
        transform,
        Projection::Perspective(PerspectiveProjection {
            near: 0.1,
            far: 50_000.0,
            fov: std::f32::consts::FRAC_PI_3,
            ..default()
        }),
        FreeFlyCamera {
            yaw,
            pitch,
            ..default()
        },
    ));

    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

/// Aplica o movimento do mouse à orientação da câmera, com pitch limitado para evitar inversão.
fn mouse_look(
    mut mouse_motion: MessageReader<MouseMotion>,
    mut cameras: Query<(&mut Transform, &mut FreeFlyCamera)>,
) {
    let Ok((mut transform, mut camera)) = cameras.single_mut() else {
        return;
    };

    let mut delta = Vec3::ZERO;
    for motion in mouse_motion.read() {
        delta.x += motion.delta.x;
        delta.y += motion.delta.y;
    }

    if delta.x == 0.0 && delta.y == 0.0 {
        return;
    }

    camera.yaw -= delta.x * camera.sensitivity;
    camera.pitch = clamp_pitch(camera.pitch - delta.y * camera.sensitivity);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
}

/// Restringe o pitch ao intervalo `[-PITCH_LIMIT, PITCH_LIMIT]`.
///
/// Tá como teste unitário para permitir teste unitário sem depender do ECS.
fn clamp_pitch(pitch: f32) -> f32 {
    pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT)
}

/// Move a câmera segundo as teclas wasd, espaço/ctrl para subir/descer e shit para sprint.
fn keyboard_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut Transform, &FreeFlyCamera)>,
) {
    let Ok((mut transform, camera)) = cameras.single_mut() else {
        return;
    };

    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction += *transform.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= *transform.forward();
    }
    if keys.pressed(KeyCode::KeyA) {
        direction += *transform.left();
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += *transform.right();
    }
    if keys.pressed(KeyCode::Space) {
        direction += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        direction -= Vec3::Y;
    }

    if direction.length_squared() == 0.0 {
        return;
    }

    let multiplier = if keys.pressed(KeyCode::ShiftLeft) {
        camera.sprint_multiplier
    } else {
        1.0
    };

    transform.translation += direction.normalize() * camera.speed * multiplier * time.delta_secs();
}

/// Libera o cursor ao pressionar ESC.
fn cursor_release(keys: Res<ButtonInput<KeyCode>>, mut cursor_options: Single<&mut CursorOptions>) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimePlugin;

    /// Testa que o pitch é corretamente limitado dentro dos limites definidos.
    #[test]
    fn clamp_pitch_restricts_pitch_within_limits() {
        let pitch = 2.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, PITCH_LIMIT);

        let pitch = -2.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, -PITCH_LIMIT);

        let pitch = 1.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, pitch);
    }

    /// Testa que segurar W descola a câmera para frente ao longo do tempo
    #[test]
    fn keyboard_movement_moves_camera_forward_when_w_pressed() {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, keyboard_movement);

        let transform = Transform::from_translation(Vec3::ZERO);
        let camera = FreeFlyCamera::default();
        let entity = app.world_mut().spawn((transform, camera)).id();

        app.update();
        let position_after_first_update = app.world().get::<Transform>(entity).unwrap().translation;

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        app.update();
        let position_after_second_update =
            app.world().get::<Transform>(entity).unwrap().translation;

        assert!(
            position_after_second_update.z < position_after_first_update.z,
            "câmera deveria se descolar para frente (-Z) ao segurar W, mas foi de {:?} para {:?}",
            position_after_first_update,
            position_after_second_update
        );
    }
}
