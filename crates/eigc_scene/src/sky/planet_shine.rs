//! Brilho refletido de Júpiter sobre a lua ativa, com base na posição do sol e de Júpiter.

use crate::sky::{SkySettings, SkyState};
use bevy::app::App;
use bevy::prelude::{
    Commands, Component, DirectionalLight, IntoScheduleConfigs, Name, Plugin, Query, Res, Startup,
    Transform, Update, Vec3, With, default, resource_exists,
};
use eigc_sim::SimSet;

/// Plugin que gerencia o brilho refletido de Júpiter sobre a lua ativa.
pub struct PlanetShinePlugin;

/// Componente que identifica a entidade da luz do brilho de Júpiter.
#[derive(Component)]
struct PlanetShineLight;

///  Plugin que registra os sistemas para criar e atualizar a luz do brilho de Júpiter.
impl Plugin for PlanetShinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_planet_shine).add_systems(
            Update,
            update_planet_shine
                .in_set(SimSet::Animate)
                .run_if(resource_exists::<SkySettings>),
        );
    }
}

/// Cria a luz direcional do brilho de Júpiter.
fn spawn_planet_shine(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 0.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::IDENTITY,
        PlanetShineLight,
        Name::new("PlanetShine"),
    ));
}

/// Atualiza a luz direcional do brilho de Júpiter com base na direção do sol e no fator de brilho.
fn update_planet_shine(
    settings: Res<SkySettings>,
    state: Res<SkyState>,
    mut q: Query<(&mut DirectionalLight, &mut Transform), With<PlanetShineLight>>,
) {
    if settings.planet_shine_max <= 0.0 {
        return;
    }

    let Ok((mut light, mut t)) = q.single_mut() else {
        return;
    };

    t.rotation = Transform::IDENTITY
        .looking_to(-state.jupiter_dir.normalize(), Vec3::Y)
        .rotation;

    light.illuminance = settings.sun_illuminance * state.planet_shine_factor;
}
