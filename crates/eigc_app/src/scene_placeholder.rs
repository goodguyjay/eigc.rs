//! Luz mínima para visualizar o terreno gerado. Esse arquivo tem que deixar de existir quando
//! o sistema de iluminação estiver implementado. (╯°□°)╯︵ ┻━┻

use bevy::prelude::{Commands, DirectionalLight, Name, Transform, Vec3};

/// Spawna uma câmera fixa olhando para o centro do terreno e uma luz direcional simples.
pub fn spawn_placeholder_light(mut commands: Commands) {
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
