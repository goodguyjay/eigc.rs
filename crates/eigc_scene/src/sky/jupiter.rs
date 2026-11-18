//! Plugin das propriedades de Júpiter no céu.

use crate::sky::shared::place_celestial_disc;
use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState};
use bevy::app::App;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{Camera3d, CameraProjection};
use bevy::light::NotShadowCaster;
use bevy::prelude::{
    AlphaMode, Assets, Color, Commands, Component, Handle, Image, IntoScheduleConfigs, Mesh,
    Mesh3d, MeshMaterial3d, Meshable, Name, Plugin, Projection, Quat, Query, Res, ResMut, Sphere,
    StandardMaterial, Transform, Update, Vec3, With, Without, any_with_component, default,
    in_state, on_message, resource_exists,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::state::state_scoped::DespawnOnExit;
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonProfile};

/// Plugin das propriedades de Júpiter no céu.
pub struct JupiterPlugin;

/// Componente que identifica a entidade de Júpiter.
#[derive(Component)]
struct Jupiter;

/// Plugin que registra os sistemas para criar e posicionar Júpiter.
impl Plugin for JupiterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            spawn_jupiter
                .run_if(on_message::<SkyAssetsLoaded>)
                .run_if(in_state(AppState::Running)),
        )
            .add_systems(
                Update,
                place_and_scale_jupiter
                    .run_if(any_with_component::<Jupiter>)
                    .run_if(resource_exists::<SkySettings>),
            );
    }
}

/// Textura de oclusão preta de 1 texel. No `StandardMaterial`, a oclusão multiplica só a luz
/// indireta (a luz ambiente, entre elas), e não a luz direta do sol.
fn black_occlusion_texture(images: &mut Assets<Image>) -> Handle<Image> {
    images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Cria a entidade de Júpiter com os materiais apropriados.
fn spawn_jupiter(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sky_assets: Res<SkyAssets>,
) {
    let mat = mats.add(StandardMaterial {
        base_color_texture: Some(sky_assets.jupiter_tex.clone()),
        base_color: Color::srgb(1.0, 1.0, 1.0),
        unlit: false,
        reflectance: 0.0,
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Opaque,
        // A luz ambiente existe para clarear as sombras do terreno. Em Júpiter, ela acenderia o
        // lado noturno do disco, que só recebe luz do sol.
        occlusion_texture: Some(black_occlusion_texture(&mut images)),
        ..default()
    });

    let sphere_mesh = meshes.add(Mesh::from(Sphere::new(1.0).mesh().uv(64, 32)));

    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(mat),
        Transform::default(),
        Jupiter,
        NoFrustumCulling,
        // Júpiter é reposicionado em relação à câmera a cada quadro. Com o sol atrás dele, a
        // esfera ficaria entre o sol e o chão e projetaria uma sombra que anda com o jogador.
        NotShadowCaster,
        Name::new("Jupiter"),
        DespawnOnExit(AppState::Running),
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

    let dir = state.jupiter_dir.normalize();
    let (position, scale) = place_celestial_disc(
        cam_t.translation,
        far,
        dir,
        profile.jupiter_angular_diameter_deg,
        eigc_common::constants::SKY_RADIUS,
    );

    t.translation = position;
    t.scale = Vec3::splat(scale);

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
    t.rotate(Quat::from_axis_angle(right, -std::f32::consts::FRAC_PI_2));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::{AssetApp, AssetPlugin, PerspectiveProjection, Startup};
    use eigc_moons::{MoonId, SkyCalibration, TerrainCalibration};

    /// App mínimo que só cria Júpiter.
    fn app_with_spawned_jupiter() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(SkyAssets {
                jupiter_tex: Handle::<Image>::default(),
                sun_tex: Handle::<Image>::default(),
                starfield_tex: Handle::<Image>::default(),
                all_loaded: true,
            })
            .add_systems(Startup, spawn_jupiter);
        app.update();
        app
    }

    /// Regressão: Júpiter projetava sombra. Com o sol atrás dele, aparecia no chão uma faixa
    /// escura a ~3 km que recuava conforme o jogador se aproximava.
    #[test]
    fn jupiter_does_not_cast_shadows() {
        let mut app = app_with_spawned_jupiter();

        let mut jupiter = app
            .world_mut()
            .query_filtered::<&NotShadowCaster, With<Jupiter>>();
        assert_eq!(jupiter.iter(app.world()).count(), 1);
    }

    /// Regressão: com a luz ambiente em 150 (para clarear as sombras do terreno), o lado noturno
    /// de Júpiter aparecia marrom e iluminado. A oclusão preta tira a luz ambiente do material
    /// sem mexer na luz do sol.
    #[test]
    fn jupiter_night_side_ignores_the_ambient_light() {
        let mut app = app_with_spawned_jupiter();

        let mut jupiter = app
            .world_mut()
            .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<Jupiter>>();
        let handle = jupiter.single(app.world()).expect("deveria existir um Júpiter").0.clone();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let occlusion = materials
            .get(&handle)
            .and_then(|material| material.occlusion_texture.clone())
            .expect("Júpiter deveria ter textura de oclusão");

        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(&occlusion)
            .expect("a textura de oclusão deveria existir");
        let data = image.data.as_ref().expect("a oclusão deveria ter dados na CPU");
        assert_eq!(data[0], 0, "o canal vermelho (oclusão) deveria ser zero");
    }

    fn test_profile(jupiter_angular_diameter_deg: f32) -> MoonProfile {
        MoonProfile {
            moon_id: MoonId::Europa,
            display_name: "Perfil de teste".to_string(),
            jupiter_angular_diameter_deg,
            terrain: TerrainCalibration {
                seed: 1,
                base_frequency: 0.001,
                feature_direction: [1.0, 0.0],
                warp_amplitude_meters: 20.0,
                perceptual_roughness: 0.5,
                reflectance: 0.3,
            },
            terrain_base_color: [1.0, 1.0, 1.0, 1.0],
            terrain_steep_color: [0.0, 0.0, 0.0, 1.0],
            walkable: true,
            sky: SkyCalibration {
                orbital_period_seconds: 1000.0,
                base_jupiter_dir: [1.0, 0.2, 0.0],
                jupiter_libration_lat_deg: 0.0,
                jupiter_libration_lon_deg: 0.0,
                jupiter_ang_radius: 0.104_72,
                sun_elevation_deg: 20.0,
                eclipse_soft_deg: 1.0,
                planet_shine_max: 0.006,
            },
        }
    }

    /// Sem `SkySettings`, `place_and_scale_jupiter` não deve rodar; com ela, deve posicionar
    /// Júpiter.
    #[test]
    fn jupiter_only_updates_with_sky_settings() {
        let mut app = App::new();

        let mut profiles = Assets::<MoonProfile>::default();
        let handle = profiles.add(test_profile(20.0));

        app.add_message::<SkyAssetsLoaded>()
            .insert_resource(SkyState {
                jupiter_dir: Vec3::new(0.0, 0.0, 1.0),
                ..default()
            })
            .insert_resource(profiles)
            .insert_resource(ActiveMoonProfileHandle(handle))
            .add_plugins(JupiterPlugin);

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
        let jupiter_entity = app.world_mut().spawn((Jupiter, known_transform)).id();

        app.update();

        assert_eq!(
            *app.world().get::<Transform>(jupiter_entity).unwrap(),
            known_transform,
            "place_and_scale_jupiter não deveria rodar sem SkySettings"
        );

        app.insert_resource(SkySettings::default());
        app.update();

        assert_ne!(
            *app.world().get::<Transform>(jupiter_entity).unwrap(),
            known_transform,
            "place_and_scale_jupiter deveria rodar com SkySettings presente"
        );
    }

    /// Testa que `place_and_scale_jupiter` posiciona, escala e orienta Júpiter corretamente.
    #[test]
    fn place_and_scale_jupiter_matches_place_celestial_disc_formula() {
        let mut app = App::new();

        let cam_translation = Vec3::new(5.0, 0.0, -5.0);
        let far = 50_000.0;
        let jupiter_dir = Vec3::new(0.0, 6.0, 8.0); // não normalizado de propósito
        let angular_diameter_deg = 20.0;

        let mut profiles = Assets::<MoonProfile>::default();
        let handle = profiles.add(test_profile(angular_diameter_deg));

        app.insert_resource(SkyState {
            jupiter_dir,
            ..default()
        })
        .insert_resource(profiles)
        .insert_resource(ActiveMoonProfileHandle(handle))
        .add_systems(Update, place_and_scale_jupiter);

        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_translation(cam_translation),
            Projection::Perspective(PerspectiveProjection { far, ..default() }),
        ));

        let jupiter_entity = app.world_mut().spawn((Jupiter, Transform::default())).id();

        app.update();

        let jupiter_transform = app.world().get::<Transform>(jupiter_entity).unwrap();

        let (expected_position, expected_scale) = place_celestial_disc(
            cam_translation,
            far,
            jupiter_dir,
            angular_diameter_deg,
            eigc_common::constants::SKY_RADIUS,
        );

        assert!(
            (jupiter_transform.translation - expected_position).length() < 1e-3,
            "posição de Júpiter {:?} não bate com o esperado {:?}",
            jupiter_transform.translation,
            expected_position
        );
        assert!(
            (jupiter_transform.scale - Vec3::splat(expected_scale)).length() < 1e-3,
            "escala de Júpiter {:?} não bate com o esperado {:?}",
            jupiter_transform.scale,
            Vec3::splat(expected_scale)
        );

        // A rotação final de -90° em `right` faz `up()` apontar para `-dir`.
        let expected_up = -jupiter_dir.normalize();
        let up = jupiter_transform.up().as_vec3();
        assert!(
            (up - expected_up).length() < 1e-3,
            "up de Júpiter {:?} não bate com o esperado {:?}",
            up,
            expected_up
        );
    }
}
