//! Voo livre ("freefly"): modo de movimento de debug, que ignora o terreno.

use bevy::prelude::{ButtonInput, Component, KeyCode, Query, Res, Time, Transform, Vec3};

/// Marca a câmera de voo livre e guarda os parâmetros de movimento. A orientação fica em
/// [`CameraLook`](crate::camera::look::CameraLook).
#[derive(Component)]
pub struct FreeFlyCamera {
    /// Velocidade de movimento da câmera, unidades por seg.
    pub speed: f32,
    /// Multiplicador de velocidade apertando shift
    pub sprint_multiplier: f32,
}

impl Default for FreeFlyCamera {
    fn default() -> Self {
        Self {
            speed: 20.0,
            sprint_multiplier: 100.0,
        }
    }
}

/// Move a câmera segundo as teclas wasd, espaço/ctrl para subir/descer e shit para sprint.
pub(super) fn free_fly_movement(
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Update};
    use bevy::time::TimePlugin;

    /// Testa que segurar W descola a câmera para frente ao longo do tempo
    #[test]
    fn free_fly_movement_moves_camera_forward_when_w_pressed() {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, free_fly_movement);

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
