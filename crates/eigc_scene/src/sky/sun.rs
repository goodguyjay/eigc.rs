//! Luz direcional do sol e disco visual no céu.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState};
use bevy::app::App;
use bevy::camera::CameraProjection;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::{
    AlphaMode, Assets, Camera3d, Color, Commands, Component, DirectionalLight, GlobalAmbientLight,
    IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Meshable, Name, Plugin, Projection, Query,
    Res, ResMut, Sphere, StandardMaterial, Transform, Update, Vec3, With, Without,
    any_with_component, default, on_message, resource_exists,
};
use eigc_sim::SimSet;

/// Plugin que gerencia a luz do sol e o disco visual no céu.
pub struct SunPlugin;

#[derive(Component)]
struct SunLight;
#[derive(Component)]
struct SunDisc;

/// Plugin que registra os sistemas para criar e atualizar a luz do sol e o disco visual.
impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_sun.run_if(on_message::<SkyAssetsLoaded>))
            .add_systems(
                Update,
                (position_sun_disc, update_sun_light)
                    .in_set(SimSet::Animate)
                    .run_if(any_with_component::<SunDisc>)
                    // `update_sun_light` lê `SkySettings` de verdade (ver assinatura abaixo);
                    // `position_sun_disc` não lê, mas usa a mesma guarda como proxy de "céu já
                    // inicializado", já que `SkyState` é `init_resource` e nunca fica ausente.
                    .run_if(resource_exists::<SkySettings>),
            );
    }
}

/// Cria a luz direcional do sol e o disco visual no céu.
fn spawn_sun(
    mut commands: Commands,
    settings: Res<SkySettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    sky_assets: Res<SkyAssets>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: settings.sun_illuminance,
            shadows_enabled: true,
            shadow_depth_bias: 0.005,
            shadow_normal_bias: 0.6,
            ..default()
        },
        Transform::IDENTITY.looking_to(-settings.base_jupiter_dir.normalize(), Vec3::Y),
        SunLight,
        Name::new("SunLight"),
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: settings.ambient_brightness,
        ..default()
    });

    let disc_mat = mats.add(StandardMaterial {
        base_color_texture: Some(sky_assets.sun_tex.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        cull_mode: None,
        ..default()
    });

    let disc_mesh = meshes.add(Mesh::from(Sphere::new(1.0).mesh().uv(32, 16)));

    commands.spawn((
        Mesh3d(disc_mesh),
        MeshMaterial3d(disc_mat),
        Transform::default(),
        SunDisc,
        Name::new("SunDisc"),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Atualiza a direção e intensidade da luz do sol com base na direção do sol e no fator de eclipse.
fn update_sun_light(
    settings: Res<SkySettings>,
    state: Res<SkyState>,
    mut q_light: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if let Ok((mut light, mut t)) = q_light.single_mut() {
        t.rotation = Transform::IDENTITY
            .looking_to(-state.sun_dir.normalize(), Vec3::Y)
            .rotation;

        light.illuminance = settings.sun_illuminance * state.eclipse_factor.clamp(0.0, 1.0);

        ambient.brightness = settings.ambient_brightness;
    }
}

/// Posiciona e escala o disco do sol com base na direção do sol e no perfil da lua ativo.
fn position_sun_disc(
    state: Res<SkyState>,
    cam_q: Query<(&Transform, &Projection), (With<Camera3d>, Without<SunDisc>)>,
    mut disc_q: Query<&mut Transform, (With<SunDisc>, Without<Camera3d>)>,
) {
    let Ok((cam_t, proj)) = cam_q.single() else {
        return;
    };
    let far = match proj {
        Projection::Perspective(p) => p.far(),
        Projection::Orthographic(o) => o.far(),
        _ => return,
    };

    let sky_r = (far * 0.85).min(eigc_common::constants::SKY_RADIUS);

    let dir_to_sun = state.sun_dir.normalize();
    let Ok(mut t) = disc_q.single_mut() else {
        return;
    };

    t.translation = cam_t.translation + dir_to_sun * sky_r;

    let theta = eigc_common::constants::SUN_ANGULAR_DIAMETER_DEG.to_radians();
    let radius = sky_r * (0.5 * theta).tan();
    t.scale = Vec3::splat(radius);

    t.look_to(-dir_to_sun, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::{
        App, Camera3d, DirectionalLight, GlobalAmbientLight, Projection, SkyAssetsLoaded,
        SkySettings, SkyState, SunDisc, SunLight, SunPlugin, Transform, Vec3, default,
    };
    use bevy::prelude::PerspectiveProjection;

    /// Sem `SkySettings`, `position_sun_disc`/`update_sun_light` não devem rodar; com ela,
    /// devem atualizar o disco e a luz (issue #27).
    #[test]
    fn sun_disc_and_light_only_update_with_sky_settings() {
        let mut app = App::new();

        app.add_message::<SkyAssetsLoaded>()
            .insert_resource(SkyState {
                sun_dir: Vec3::new(1.0, 0.0, 0.0),
                eclipse_factor: 1.0,
                ..default()
            })
            .insert_resource(GlobalAmbientLight {
                brightness: 0.0,
                ..default()
            })
            .add_plugins(SunPlugin);

        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_translation(Vec3::ZERO),
            Projection::Perspective(PerspectiveProjection {
                far: 50_000.0,
                ..default()
            }),
        ));

        let known_transform =
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)).with_scale(Vec3::splat(5.0));
        let disc_entity = app.world_mut().spawn((SunDisc, known_transform)).id();

        let light_entity = app
            .world_mut()
            .spawn((
                SunLight,
                DirectionalLight {
                    illuminance: 123.0,
                    ..default()
                },
                Transform::IDENTITY,
            ))
            .id();

        // Fase 1: sem SkySettings, nada deveria rodar.
        app.update();

        assert_eq!(
            *app.world().get::<Transform>(disc_entity).unwrap(),
            known_transform,
            "position_sun_disc não deveria rodar sem SkySettings"
        );
        assert_eq!(
            app.world()
                .get::<DirectionalLight>(light_entity)
                .unwrap()
                .illuminance,
            123.0,
            "update_sun_light não deveria rodar sem SkySettings"
        );

        // Fase 2: com SkySettings, os dois devem atualizar.
        app.insert_resource(SkySettings {
            sun_illuminance: 999.0,
            ambient_brightness: 0.5,
            ..default()
        });
        app.update();

        assert_ne!(
            *app.world().get::<Transform>(disc_entity).unwrap(),
            known_transform,
            "position_sun_disc deveria rodar com SkySettings presente"
        );
        assert_ne!(
            app.world()
                .get::<DirectionalLight>(light_entity)
                .unwrap()
                .illuminance,
            123.0,
            "update_sun_light deveria rodar com SkySettings presente"
        );
    }
}
