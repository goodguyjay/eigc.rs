//! Domo de estrelas de fundo, com escurecimento perto do sol.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkyState};
use bevy::app::App;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::{
    AlphaMode, Assets, Camera3d, Color, Commands, Component, IntoScheduleConfigs, Mesh, Mesh3d,
    MeshMaterial3d, Meshable, Name, Plugin, Projection, Query, Res, ResMut, Sphere,
    StandardMaterial, Transform, Update, Vec3, With, Without, any_with_component, default,
    on_message,
};

/// Plugin que gerencia o domo de estrelas de fundo.
pub struct StarfieldPlugin;

/// Componente que identifica a entidade do domo de estrelas.
#[derive(Component)]
struct StarDome;

///  Plugin que registra os sistemas para criar e atualizar o domo de estrelas.
impl Plugin for StarfieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            spawn_starfield.run_if(on_message::<SkyAssetsLoaded>),
        )
        .add_systems(
            Update,
            (track_camera, dim_stars_near_sun).run_if(any_with_component::<StarDome>),
        );
    }
}

/// Cria o domo de estrelas.
fn spawn_starfield(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    sky_assets: Res<SkyAssets>,
) {
    let dome_mesh = meshes.add(Mesh::from(Sphere::new(1.0).mesh().uv(128, 64)));
    let dome_mat = mats.add(StandardMaterial {
        base_color_texture: Some(sky_assets.starfield_tex.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(dome_mesh),
        MeshMaterial3d(dome_mat),
        Transform::from_scale(Vec3::splat(20_000.0)),
        StarDome,
        NoFrustumCulling,
        Name::new("Domo de Estrelas"),
    ));
}

/// Mantém o domo de estrelas na posição da câmera, para que ele não se mova com o parallax.
fn track_camera(
    cam_q: Query<&Transform, (With<Camera3d>, Without<StarDome>)>,
    mut dome_q: Query<&mut Transform, (With<StarDome>, Without<Camera3d>)>,
) {
    let Ok(cam) = cam_q.single() else {
        return;
    };
    let Ok(mut t) = dome_q.single_mut() else {
        return;
    };

    t.translation = cam.translation;
}

/// Escurece o domo de estrelas quando a câmera está olhando perto do sol, para reduzir o brilho das estrelas.
fn dim_stars_near_sun(
    state: Res<SkyState>,
    cam_q: Query<(&Transform, &Projection), (With<Camera3d>, Without<StarDome>)>,
    star_q: Query<&MeshMaterial3d<StandardMaterial>, (With<StarDome>, Without<Camera3d>)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((cam, proj)) = cam_q.single() else {
        return;
    };
    let Ok(star_mat) = star_q.single() else {
        return;
    };

    let view_dir = cam.forward().normalize();
    let sun_dir = state.sun_dir.normalize();
    // separação angular entre o centro da visão e o sol
    let sep = view_dir.dot(sun_dir).clamp(-1.0, 1.0).acos();

    // cone local de glare ao redor do sol
    let inner = 3.0_f32.to_radians();
    let outer = 12.0_f32.to_radians();
    let local = eigc_common::math::smoothstep(inner, outer, sep);
    let local = 0.15 + 0.85 * local;

    let fov = match proj {
        Projection::Perspective(p) => p.fov,
        Projection::Orthographic(_) => std::f32::consts::FRAC_PI_2,
        _ => return,
    };

    let g0 = fov * 0.15;
    let g1 = fov * 0.5;
    let global = eigc_common::math::smoothstep(g0, g1, sep);
    let global = 0.55 + 0.45 * global;

    let brightness = local * global;

    if let Some(mat) = mats.get_mut(&star_mat.0) {
        mat.base_color = Color::linear_rgb(brightness, brightness, brightness);
    }
}
