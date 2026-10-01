//! Plugin das propriedades de Júpiter no céu.

use crate::sky::shared::place_celestial_disc;
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
    use super::{
        ActiveMoonProfileHandle, App, Assets, Camera3d, Jupiter, MoonProfile, Projection, SkyState,
        Transform, Update, Vec3, default, place_and_scale_jupiter, place_celestial_disc,
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

    /// Testa que `place_and_scale_jupiter` posiciona e escala Júpiter exatamente como
    /// `place_celestial_disc` calcularia para os mesmos parâmetros. Guarda de regressão da
    /// refatoração que trocou a fórmula duplicada por uma chamada à função compartilhada.
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

        // `place_and_scale_jupiter` termina com `t.rotate(Quat::from_axis_angle(right,
        // -FRAC_PI_2))`, que gira o frame -90° em torno do próprio eixo `right`. Essa rotação
        // troca `forward` por `up` (com sinal): `up_novo = forward_antigo` e
        // `forward_novo = -up_antigo`. Como `forward_antigo` (o que entrou no `look_to`) é
        // exatamente `-dir`, isso dá uma verificação independente da orientação final sem
        // precisar reproduzir a libração/obliquidade: se essa rotação de correção (do PR #33)
        // for removida ou trocada de sinal, `up()` deixa de bater com `-dir` e o teste falha.
        let expected_up = -jupiter_dir.normalize();
        let up = jupiter_transform.up().as_vec3();
        assert!(
            (up - expected_up).length() < 1e-3,
            "up de Júpiter {:?} não bate com o esperado {:?} (regressão da correção de orientação do PR #33)",
            up,
            expected_up
        );
    }
}
