//! Plugin das propriedades de Júpiter no céu.

use crate::sky::{SkyAssets, SkyAssetsLoaded, SkySettings, SkyState};
use bevy::app::App;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{Camera3d, CameraProjection};
use bevy::prelude::{
    AlphaMode, Assets, Color, Commands, Component, IntoScheduleConfigs, Mesh, Mesh3d,
    MeshMaterial3d, Meshable, Name, Plugin, Projection, Quat, Query, Res, ResMut, Sphere,
    StandardMaterial, Transform, Update, Vec3, With, Without, any_with_component, default,
    on_message, resource_exists,
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
                place_and_scale_jupiter
                    .run_if(any_with_component::<Jupiter>)
                    // `place_and_scale_jupiter` não lê `SkySettings` diretamente — a guarda
                    // funciona como proxy de "céu já inicializado", já que `SkyState` é
                    // `init_resource` e nunca fica ausente.
                    .run_if(resource_exists::<SkySettings>),
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
    t.rotate(Quat::from_axis_angle(right, -std::f32::consts::FRAC_PI_2));
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveMoonProfileHandle, App, Assets, Camera3d, Jupiter, JupiterPlugin, MoonProfile,
        Projection, SkyAssetsLoaded, SkySettings, SkyState, Transform, Vec3, default,
    };
    use bevy::prelude::PerspectiveProjection;
    use eigc_moons::{MoonId, SkyCalibration, TerrainCalibration};

    fn test_profile(jupiter_angular_diameter_deg: f32) -> MoonProfile {
        MoonProfile {
            moon_id: MoonId::Europa,
            display_name: "Perfil de teste".to_string(),
            jupiter_angular_diameter_deg,
            terrain: TerrainCalibration {
                seed: 1,
                base_frequency: 0.001,
                feature_direction: [1.0, 0.0],
                vertical_amplitude_meters: 10.0,
                warp_amplitude_meters: 20.0,
                perceptual_roughness: 0.5,
                reflectance: 0.3,
            },
            terrain_base_color: [1.0, 1.0, 1.0, 1.0],
            terrain_valley_color: [0.0, 0.0, 0.0, 1.0],
            walkable: true,
            sky: SkyCalibration {
                orbital_period_seconds: 1000.0,
                base_sun_dir: [0.0, 0.3, -1.0],
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
    /// Júpiter (issue #27).
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

        // Fase 1: sem SkySettings, nada deveria rodar.
        app.update();

        assert_eq!(
            *app.world().get::<Transform>(jupiter_entity).unwrap(),
            known_transform,
            "place_and_scale_jupiter não deveria rodar sem SkySettings"
        );

        // Fase 2: com SkySettings, deveria rodar.
        app.insert_resource(SkySettings::default());
        app.update();

        assert_ne!(
            *app.world().get::<Transform>(jupiter_entity).unwrap(),
            known_transform,
            "place_and_scale_jupiter deveria rodar com SkySettings presente"
        );
    }
}
