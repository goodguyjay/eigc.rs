//! Domo de estrelas de fundo, com escurecimento perto do sol.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkyState};
use bevy::app::App;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
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
        NotShadowCaster,
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

    let fov = match proj {
        Projection::Perspective(p) => p.fov,
        Projection::Orthographic(_) => std::f32::consts::FRAC_PI_2,
        _ => return,
    };

    let view_dir = cam.forward().normalize();
    let sun_dir = state.sun_dir.normalize();
    let sep = view_dir.dot(sun_dir).clamp(-1.0, 1.0).acos();
    let brightness = star_brightness(sep, fov, state.eclipse_factor);

    if let Some(mat) = mats.get_mut(&star_mat.0) {
        mat.base_color = Color::linear_rgb(brightness, brightness, brightness);
    }
}

/// Brilho do domo de estrelas, de 0 a 1, dado o ângulo entre o centro da visão e o sol, o fov
/// vertical (ambos em radianos) e o fator de eclipse.
///
/// O glare do sol escurece as estrelas perto dele e, mais de leve, a tela toda quando o sol
/// está no campo de visão. No eclipse o sol está atrás de Júpiter e não ofusca nada, então o
/// glare some proporcionalmente ao `eclipse_factor`.
fn star_brightness(sep_to_sun: f32, fov: f32, eclipse_factor: f32) -> f32 {
    let inner = 3.0_f32.to_radians();
    let outer = 12.0_f32.to_radians();
    let local = 0.15 + 0.85 * eigc_common::math::smoothstep(inner, outer, sep_to_sun);

    let global = 0.55 + 0.45 * eigc_common::math::smoothstep(fov * 0.15, fov * 0.5, sep_to_sun);

    let glare_brightness = local * global;
    1.0 - (1.0 - glare_brightness) * eclipse_factor.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::{App, AssetApp, AssetPlugin, Handle, Image, Mesh, Startup};

    /// O domo envolve a câmera e a luz do sol vem de fora dele. Se projetar sombra, o chão dentro
    /// do alcance das cascatas fica todo preto.
    #[test]
    fn star_dome_does_not_cast_shadows() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(SkyAssets {
                jupiter_tex: Handle::<Image>::default(),
                sun_tex: Handle::<Image>::default(),
                starfield_tex: Handle::<Image>::default(),
                all_loaded: true,
            })
            .add_systems(Startup, spawn_starfield);

        app.update();

        let mut dome = app
            .world_mut()
            .query_filtered::<&NotShadowCaster, With<StarDome>>();
        assert_eq!(dome.iter(app.world()).count(), 1);
    }

    /// Fov vertical padrão de perspectiva do Bevy (45 graus).
    const FOV: f32 = std::f32::consts::FRAC_PI_4;

    /// Testa que, sem eclipse, olhar para o sol escurece as estrelas e olhar para longe não.
    #[test]
    fn star_brightness_dims_near_sun_without_eclipse() {
        let looking_at_sun = star_brightness(0.0, FOV, 1.0);
        let looking_away = star_brightness(std::f32::consts::PI, FOV, 1.0);

        assert!(looking_at_sun < 0.2, "olhando para o sol: {looking_at_sun}");
        assert_eq!(looking_away, 1.0);
    }

    /// Regressão: o glare era calculado sem olhar o eclipse, e as estrelas em volta de Júpiter
    /// ficavam apagadas justo quando o sol estava escondido atrás dele.
    #[test]
    fn star_brightness_is_full_during_total_eclipse() {
        assert_eq!(star_brightness(0.0, FOV, 0.0), 1.0);
    }

    /// Testa que durante a entrada no eclipse o brilho fica entre o glare e o brilho cheio.
    #[test]
    fn star_brightness_blends_glare_away_during_partial_eclipse() {
        let glare = star_brightness(0.0, FOV, 1.0);
        let partial = star_brightness(0.0, FOV, 0.5);

        assert!(glare < partial && partial < 1.0, "glare {glare}, parcial {partial}");
    }
}
