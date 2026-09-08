//! Luz direcional do sol e disco visual no céu.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState};
use bevy::app::App;
use bevy::camera::CameraProjection;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::{
    AlphaMode, Assets, Camera3d, Color, Commands, Component, DirectionalLight,
    GlobalAmbientLight, IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Meshable, Name, Plugin,
    Projection, Query, Res, ResMut, Sphere, StandardMaterial, Transform, Update, Vec3, With,
    Without, any_with_component, default, on_message,
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
