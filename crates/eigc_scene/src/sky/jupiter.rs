//! Plugin das propriedades de Júpiter no céu.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkyState};
use bevy::app::App;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{Camera3d, CameraProjection};
use bevy::prelude::{
    AlphaMode, Assets, Color, Commands, Component, IntoScheduleConfigs, Mesh, Mesh3d,
    MeshMaterial3d, Meshable, Name, Plugin, Projection, Quat, Query, Res, ResMut, Sphere,
    StandardMaterial, Transform, Update, Vec3, With, Without, any_with_component, default,
    on_message,
};
use eigc_moons::{ActiveMoonProfileHandle, MoonProfile};

/// Plugin das propriedades de Júpiter no céu.
pub struct JupiterPlugin;

/// Componente que identifica a entidade de Júpiter.
#[derive(Component)]
struct Jupiter;

/// Plugin que registra os sistemas para criar e posicionar Júpiter.
impl Plugin for JupiterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_jupiter.run_if(on_message::<SkyAssetsLoaded>))
            .add_systems(
                Update,
                place_and_scale_jupiter.run_if(any_with_component::<Jupiter>),
            );
    }
}

/// Cria a entidade de Júpiter com os materiais apropriados.
fn spawn_jupiter(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    sky_assets: Res<SkyAssets>,
) {
    let mat = mats.add(StandardMaterial {
        base_color_texture: Some(sky_assets.jupiter_tex.clone()),
        base_color: Color::srgb(1.0, 1.0, 1.0),
        unlit: false,
        reflectance: 0.0,
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    let sphere_mesh = meshes.add(Mesh::from(Sphere::new(1.0).mesh().uv(64, 32)));

    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(mat),
        Transform::default(),
        Jupiter,
        NoFrustumCulling,
        Name::new("Jupiter"),
    ));
}

/// Posiciona e escala Júpiter com base na direção do sol e no perfil da lua ativo.
fn place_and_scale_jupiter(
    state: Res<SkyState>,
    active: Res<ActiveMoonProfileHandle>,
    profiles: Res<Assets<MoonProfile>>,
    cam_q: Query<(&Transform, &Projection), (With<Camera3d>, Without<Jupiter>)>,
    mut jup_q: Query<&mut Transform, (With<Jupiter>, Without<Camera3d>)>,
) {
    let Some(profile) = profiles.get(&active.0) else {
        return;
    };
    let Ok((cam_t, proj)) = cam_q.single() else {
        return;
    };
    let Ok(mut t) = jup_q.single_mut() else {
        return;
    };

    let far = match proj {
        Projection::Perspective(p) => p.far(),
        Projection::Orthographic(o) => o.far(),
        _ => return,
    };
    let sky_r = (far * 0.85).min(eigc_common::constants::SKY_RADIUS);

    let dir = state.jupiter_dir.normalize();
    t.translation = cam_t.translation + dir * sky_r;

    let theta = profile.jupiter_angular_diameter_deg.to_radians();
    let radius = sky_r * (0.5 * theta).tan();
    t.scale = Vec3::splat(radius);

    let forward = (-dir).normalize_or_zero();
    let mut up_hint = Vec3::Y;
    let axis = up_hint.cross(forward).normalize_or_zero();
    if axis.length_squared() > 0.0 {
        up_hint = Quat::from_axis_angle(
            axis,
            eigc_common::constants::JUPITER_OBLIQUITY_DEG.to_radians(),
        ) * up_hint;
    }

    t.look_to(forward, up_hint);

    let right = t.right().as_vec3();

    t.rotate(Quat::from_axis_angle(right, -std::f32::consts::FRAC_2_PI));
}
