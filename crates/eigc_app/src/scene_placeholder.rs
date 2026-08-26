//! Câmera e luz mínimas para visualizar o terreno gerado. Migrar para eigc_scene quando esse crate
//! ganhar céu.

use bevy::prelude::{Camera3d, Commands, DirectionalLight, Name, Transform, Vec3};

/// Spawna uma câmera fixa olhando para o centro do terreno e uma luz direcional simples.
pub fn spawn_placeholder_camera_and_light(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 400.0, 800.0)).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Câmera Placeholder"),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::from_translation(Vec3::new(200.0, 300.0, 200.0)).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Sol Placeholder"),
    ));
}
