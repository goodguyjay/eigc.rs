//! Câmera em primeira pessoa: caminhada no plano horizontal com o olho a altura constante sobre
//! o terreno.

use crate::camera::look::CameraLook;
use bevy::prelude::{ButtonInput, Component, KeyCode, Quat, Query, Res, Time, Transform, Vec3};
use eigc_terrain::pipeline::HeightResource;

/// Parâmetros da câmera em primeira pessoa. A orientação fica em [`CameraLook`].
#[derive(Component)]
pub struct FirstPersonCamera {
    /// Velocidade de caminhada, em metros por segundo.
    pub walk_speed: f32,
    /// Multiplicador de velocidade apertando shift.
    pub sprint_multiplier: f32,
    /// Altura do olho acima do terreno, em metros.
    pub eye_height: f32,
}

impl Default for FirstPersonCamera {
    fn default() -> Self {
        Self {
            walk_speed: 2.0,
            sprint_multiplier: 4.0,
            eye_height: 1.75,
        }
    }
}

/// Direção de caminhada no plano horizontal (XZ), a partir do yaw e dos eixos de entrada.
///
/// Ignora o pitch de propósito: olhar para baixo não pode enfiar a câmera no chão nem desacelerar
/// o passo. Retorna vetor nulo sem entrada, ou com eixos que se anulam.
fn walk_direction(yaw: f32, forward_axis: f32, strafe_axis: f32) -> Vec3 {
    let rotation = Quat::from_rotation_y(yaw);
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    (forward * forward_axis + right * strafe_axis).normalize_or_zero()
}

/// Altura do olho no mundo para um terreno em `ground_y`.
fn eye_y(ground_y: f32, eye_height: f32) -> f32 {
    ground_y + eye_height
}

/// Converte um par de teclas opostas em um eixo de entrada: +1, -1 ou 0.
fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(positive) - f32::from(negative)
}

/// Caminha segundo as teclas wasd e shift para sprint, só no plano horizontal.
pub(super) fn walk_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut Transform, &CameraLook, &FirstPersonCamera)>,
) {
    let Ok((mut transform, look, camera)) = cameras.single_mut() else {
        return;
    };

    let direction = walk_direction(
        look.yaw,
        axis(keys.pressed(KeyCode::KeyW), keys.pressed(KeyCode::KeyS)),
        axis(keys.pressed(KeyCode::KeyD), keys.pressed(KeyCode::KeyA)),
    );
    if direction == Vec3::ZERO {
        return;
    }

    let multiplier = if keys.pressed(KeyCode::ShiftLeft) {
        camera.sprint_multiplier
    } else {
        1.0
    };

    transform.translation += direction * camera.walk_speed * multiplier * time.delta_secs();
}

/// Mantém o olho à altura `eye_height` acima do terreno, lendo a função de altura e não a malha.
///
/// Precisa de `HeightResource`, que o terreno insere ao entrar em `Running`: quem registra este
/// sistema deve usar `run_if(resource_exists::<HeightResource>)`.
pub(super) fn follow_ground(
    height: Res<HeightResource>,
    mut cameras: Query<(&mut Transform, &FirstPersonCamera)>,
) {
    let Ok((mut transform, camera)) = cameras.single_mut() else {
        return;
    };

    let ground_y = height
        .0
        .height_at(transform.translation.x, transform.translation.z);
    transform.translation.y = eye_y(ground_y, camera.eye_height);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Entity, Update, default};
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use eigc_terrain::height::{HeightSource, arc};
    use std::f32::consts::FRAC_PI_2;
    use std::time::Duration;

    /// Terreno inclinado só para teste: sobe 0,1 m por metro andado em -Z.
    struct Ramp;
    impl HeightSource for Ramp {
        fn height_at(&self, _x: f32, z: f32) -> f32 {
            -0.1 * z
        }
    }

    /// Testa que sem rotação "frente" é -Z e "direita" é +X.
    #[test]
    fn walk_direction_at_zero_yaw_goes_forward_along_negative_z() {
        assert!(walk_direction(0.0, 1.0, 0.0).abs_diff_eq(Vec3::NEG_Z, 1e-6));
        assert!(walk_direction(0.0, 0.0, 1.0).abs_diff_eq(Vec3::X, 1e-6));
    }

    /// Testa que o yaw gira a direção de caminhada: com 90 graus, frente vira -X.
    #[test]
    fn walk_direction_rotates_with_yaw() {
        assert!(walk_direction(FRAC_PI_2, 1.0, 0.0).abs_diff_eq(Vec3::NEG_X, 1e-6));
    }

    /// Testa que andar na diagonal não é mais rápido que andar reto.
    #[test]
    fn walk_direction_is_normalized_on_diagonals() {
        let direction = walk_direction(0.3, 1.0, 1.0);
        assert!((direction.length() - 1.0).abs() < 1e-6);
    }

    /// Testa que sem entrada, ou com teclas opostas juntas, a direção é nula.
    #[test]
    fn walk_direction_is_zero_without_net_input() {
        assert_eq!(walk_direction(1.2, 0.0, 0.0), Vec3::ZERO);
        assert_eq!(
            walk_direction(1.2, axis(true, true), axis(true, true)),
            Vec3::ZERO
        );
    }

    /// Testa que a altura do olho é a do terreno somada à altura do olho.
    #[test]
    fn eye_y_adds_eye_height_to_ground() {
        assert!((eye_y(-1.45, 1.75) - 0.3).abs() < 1e-5);
    }

    /// App com relógio de 100 ms por quadro e uma câmera em primeira pessoa na origem.
    fn walking_app(pitch: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, walk_movement);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(0.0, 7.0, 0.0)),
                CameraLook { pitch, ..default() },
                FirstPersonCamera::default(),
            ))
            .id();
        // o primeiro quadro tem delta zero
        app.update();
        (app, entity)
    }

    /// Testa que olhar para baixo não muda a altura nem a velocidade horizontal: a caminhada
    /// segue o yaw, não o `forward()` da câmera.
    #[test]
    fn walk_movement_ignores_pitch() {
        let mut distances = Vec::new();
        for pitch in [0.0, -1.2] {
            let (mut app, entity) = walking_app(pitch);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyW);
            app.update();

            let position = app.world().get::<Transform>(entity).unwrap().translation;
            assert_eq!(position.y, 7.0, "a altura mudou com pitch {pitch}");
            distances.push(position.x.hypot(position.z));
        }
        assert!(
            (distances[0] - distances[1]).abs() < 1e-5,
            "a velocidade dependeu do pitch: {distances:?}"
        );
    }

    /// Testa que o sistema coloca o olho na altura do terreno mais a altura do olho.
    #[test]
    fn follow_ground_places_eye_above_terrain() {
        let mut app = App::new();
        app.insert_resource(HeightResource(arc(Ramp)))
            .add_systems(Update, follow_ground);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(3.0, 99.0, -20.0)),
                FirstPersonCamera::default(),
            ))
            .id();

        app.update();

        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(
            (position.y - (2.0 + 1.75)).abs() < 1e-5,
            "y foi {}",
            position.y
        );
        assert_eq!((position.x, position.z), (3.0, -20.0));
    }
}
