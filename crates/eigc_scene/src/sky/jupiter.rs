//! Plugin das propriedades de Júpiter no céu.

use crate::sky::shared::place_celestial_disc;
use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState, animate_sky_physical};
use bevy::app::App;
use bevy::asset::Asset;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{Camera3d, CameraProjection};
use bevy::light::NotShadowCaster;
use bevy::math::Vec4;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::{
    AlphaMode, Assets, Color, Commands, Component, IntoScheduleConfigs, Mesh, Mesh3d,
    MeshMaterial3d, Meshable, Name, Plugin, Projection, Quat, Query, Res, ResMut, Sphere,
    StandardMaterial, Transform, Update, Vec3, With, Without, any_with_component, default,
    in_state, on_message, resource_exists,
};
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::state::state_scoped::DespawnOnExit;
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonProfile};

/// Caminho (relativo à raiz de assets) do shader de iluminação própria de Júpiter.
const JUPITER_SHADER_PATH: &str = "shaders/jupiter_phase.wgsl";

/// Material de Júpiter: o `StandardMaterial` dá a textura, e a extensão calcula a luz do sol.
pub type JupiterMaterial = ExtendedMaterial<StandardMaterial, JupiterShading>;

/// Iluminação própria de Júpiter: só o sol, sem as luzes da cena. A luz ambiente acenderia o lado
/// noturno, e a `SunLight` se apaga com o sol abaixo do horizonte do recorte, mas o sol continua
/// iluminando Júpiter (que fica cheio à noite).
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct JupiterShading {
    /// `xyz`: direção para o sol, no espaço do mundo. `w`: iluminância do sol, em lux.
    #[uniform(100)]
    pub sun: Vec4,
}

impl MaterialExtension for JupiterShading {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(JUPITER_SHADER_PATH.into())
    }
}

/// Plugin das propriedades de Júpiter no céu.
pub struct JupiterPlugin;

/// Componente que identifica a entidade de Júpiter.
#[derive(Component)]
struct Jupiter;

/// Plugin que registra os sistemas para criar e posicionar Júpiter. O `JupiterMaterial` é
/// registrado pelo `SkyPlugin`.
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
                (
                    place_and_scale_jupiter,
                    light_jupiter_with_the_sun
                        .after(animate_sky_physical)
                        .run_if(resource_exists::<Assets<JupiterMaterial>>),
                )
                    .run_if(any_with_component::<Jupiter>)
                    .run_if(resource_exists::<SkySettings>),
            );
    }
}

/// Cria a entidade de Júpiter com os materiais apropriados.
fn spawn_jupiter(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<JupiterMaterial>>,
    sky_assets: Res<SkyAssets>,
) {
    let mat = mats.add(JupiterMaterial {
        base: StandardMaterial {
            base_color_texture: Some(sky_assets.jupiter_tex.clone()),
            base_color: Color::srgb(1.0, 1.0, 1.0),
            reflectance: 0.0,
            perceptual_roughness: 1.0,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        },
        extension: JupiterShading { sun: Vec4::ZERO },
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

/// Uniform do sol para o shader de Júpiter: a direção para o sol e a iluminância cheia, sem o
/// fator de dia do recorte nem o eclipse (que é a sombra de Júpiter sobre a lua, não sobre ele).
fn jupiter_sun_uniform(sun_dir: Vec3, sun_illuminance: f32) -> Vec4 {
    sun_dir.normalize_or_zero().extend(sun_illuminance)
}

/// Atualiza a direção do sol no material de Júpiter, a partir de `SkyState` (dinâmico).
fn light_jupiter_with_the_sun(
    settings: Res<SkySettings>,
    state: Res<SkyState>,
    jup_q: Query<&MeshMaterial3d<JupiterMaterial>, With<Jupiter>>,
    mut mats: ResMut<Assets<JupiterMaterial>>,
) {
    let Ok(material) = jup_q.single() else {
        return;
    };
    if let Some(material) = mats.get_mut(&material.0) {
        material.extension.sun = jupiter_sun_uniform(state.sun_dir, settings.sun_illuminance);
    }
}

/// Posiciona e escala Júpiter com base na direção do sol e no perfil da lua ativo.
fn place_and_scale_jupiter(
    settings: Res<SkySettings>,
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
    let mut up_hint = settings.celestial_north.normalize_or(Vec3::Y);
    let axis = up_hint.cross(forward).normalize_or_zero();
    if axis.length_squared() > 0.0 {
        up_hint = Quat::from_axis_angle(axis, settings.jupiter_axis_tilt) * up_hint;
    }

    t.look_to(forward, up_hint);

    let right = t.right().as_vec3();
    t.rotate(Quat::from_axis_angle(right, -std::f32::consts::FRAC_PI_2));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::{AssetApp, AssetPlugin, Handle, Image, PerspectiveProjection, Startup};
    use eigc_moons::{MoonId, SkyCalibration, SkyGeometry, TerrainCalibration};

    /// App mínimo que só cria Júpiter.
    fn app_with_spawned_jupiter() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<JupiterMaterial>()
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

    /// Regressão: com o sol abaixo do horizonte do recorte, a `SunLight` se apaga, e Júpiter
    /// (iluminado por ela) virava um disco preto à noite, quando deveria estar cheio. O material
    /// próprio recebe o sol com a iluminância cheia, mesmo à noite e no eclipse.
    #[test]
    fn jupiter_stays_lit_by_the_sun_at_night() {
        let mut app = app_with_spawned_jupiter();
        let settings = SkySettings {
            sun_illuminance: 4500.0,
            ..default()
        };
        let sun_dir = Vec3::new(0.0, -0.5, 1.0);
        app.insert_resource(settings)
            .insert_resource(SkyState {
                sun_dir,
                daylight_factor: 0.0,
                eclipse_factor: 0.0,
                ..default()
            })
            .add_systems(Update, light_jupiter_with_the_sun);
        app.update();

        let mut jupiter = app
            .world_mut()
            .query_filtered::<&MeshMaterial3d<JupiterMaterial>, With<Jupiter>>();
        let handle = jupiter.single(app.world()).expect("deveria existir um Júpiter").0.clone();
        let sun = app
            .world()
            .resource::<Assets<JupiterMaterial>>()
            .get(&handle)
            .expect("o material de Júpiter deveria existir")
            .extension
            .sun;

        assert!((sun.truncate() - sun_dir.normalize()).length() < 1e-6, "direção {sun:?}");
        assert_eq!(sun.w, 4500.0);
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
                geometry: SkyGeometry::Fixed {
                    base_jupiter_dir: [1.0, 0.2, 0.0],
                    sun_elevation_deg: 20.0,
                },
                jupiter_libration_lat_deg: 0.0,
                jupiter_libration_lon_deg: 0.0,
                jupiter_ang_radius: 0.104_72,
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
        .insert_resource(SkySettings::default())
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
