//! Luz direcional do sol e disco visual no céu.

use crate::sky::shared::place_celestial_disc;
use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState};
use bevy::app::App;
use bevy::camera::CameraProjection;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::{
    AlphaMode, Assets, Camera3d, Color, Commands, Component, DirectionalLight, GlobalAmbientLight,
    IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Meshable, Name, Plugin, Projection, Query,
    Res, ResMut, Sphere, StandardMaterial, Transform, Update, Vec3, With, Without,
    any_with_component, default, on_message,
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
                    .run_if(any_with_component::<SunDisc>),
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

    let Ok(mut t) = disc_q.single_mut() else {
        return;
    };

    let dir_to_sun = state.sun_dir.normalize();
    let (position, scale) = place_celestial_disc(
        cam_t.translation,
        far,
        dir_to_sun,
        eigc_common::constants::SUN_ANGULAR_DIAMETER_DEG,
        eigc_common::constants::SKY_RADIUS,
    );

    t.translation = position;
    t.scale = Vec3::splat(scale);

    t.look_to(-dir_to_sun, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::PerspectiveProjection;

    /// Testa que `position_sun_disc` posiciona, escala e orienta o disco do sol corretamente.
    #[test]
    fn position_sun_disc_matches_place_celestial_disc_formula() {
        let mut app = App::new();

        let cam_translation = Vec3::new(10.0, 20.0, 30.0);
        let far = 50_000.0;
        let sun_dir = Vec3::new(3.0, 0.0, 4.0); // não normalizado de propósito

        app.insert_resource(SkyState {
            sun_dir,
            ..default()
        })
        .add_systems(Update, position_sun_disc);

        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_translation(cam_translation),
            Projection::Perspective(PerspectiveProjection { far, ..default() }),
        ));

        let disc_entity = app.world_mut().spawn((SunDisc, Transform::default())).id();

        app.update();

        let disc_transform = app.world().get::<Transform>(disc_entity).unwrap();

        let (expected_position, expected_scale) = place_celestial_disc(
            cam_translation,
            far,
            sun_dir,
            eigc_common::constants::SUN_ANGULAR_DIAMETER_DEG,
            eigc_common::constants::SKY_RADIUS,
        );

        assert!(
            (disc_transform.translation - expected_position).length() < 1e-3,
            "posição do disco do sol {:?} não bate com o esperado {:?}",
            disc_transform.translation,
            expected_position
        );
        assert!(
            (disc_transform.scale - Vec3::splat(expected_scale)).length() < 1e-3,
            "escala do disco do sol {:?} não bate com o esperado {:?}",
            disc_transform.scale,
            Vec3::splat(expected_scale)
        );

        // `position_sun_disc` termina com `t.look_to(-dir_to_sun, Vec3::Y)`, então o disco deve
        // encarar de volta a câmera. `sun_dir` entra não normalizado de propósito — confirma que
        // a normalização interna (`state.sun_dir.normalize()`) está de fato sendo aplicada antes
        // do look_to, não só na posição.
        let expected_forward = -sun_dir.normalize();
        let forward = disc_transform.forward().as_vec3();
        assert!(
            (forward - expected_forward).length() < 1e-3,
            "forward do disco do sol {:?} não bate com o esperado {:?}",
            forward,
            expected_forward
        );
    }
}
