//! Animação de abertura: cena 3D de Júpiter, as quatro luas galileanas e suas órbitas, numa
//! câmera e render layer próprios, separados da cena principal do app; por cima, a UI da luz
//! central, do véu e do clarão final que revela a cena principal na transição.

use std::f32::consts::FRAC_PI_2;

use bevy::app::App;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, Camera3d, ClearColorConfig};
use bevy::light::{DirectionalLight, NotShadowCaster};
use bevy::prelude::{
    Alpha, AlphaMode, AppExtStates, AppGizmoBuilder, AssetServer, Assets, ButtonInput, Color,
    Commands, Component, Entity, GizmoConfigGroup, GizmoConfigStore, Gizmos, Image,
    IntoScheduleConfigs, Isometry3d, KeyCode, Mesh, Mesh3d, MeshMaterial3d, Meshable, MouseButton,
    NextState, OnEnter, Plugin, Quat, Query, Reflect, Res, ResMut, Resource, Sphere,
    StandardMaterial, Startup, States, Time, Transform, Update, Vec3, With, Without, default,
    in_state,
};
use eigc_common::environment::Environment;

mod overlay;
mod texture;

const INTRO_LAYER: usize = 1;

const ZOOM_START: f32 = 4.2;
const ZOOM_END: f32 = 6.3;
const TRANSITION: f32 = 6.0;
const DURATION: f32 = 6.9;
const SKIP_TARGET: f32 = ZOOM_START - 0.3;
const REVEAL_TIME: f32 = 0.6;
const MAX_FRAME_DELTA: f32 = 1.0 / 30.0;

const CAMERA_ELEVATION: f32 = 0.3257;
const START_DISTANCE: f32 = 17.3;
const END_DISTANCE: f32 = 1.6;

const JUPITER_OBLATENESS: f32 = 0.935;
const JUPITER_SPIN: f32 = 0.35;
const MAX_ORBIT: f32 = 6.3;
const MOON_SCALE: f32 = 0.186;
const IO_ANGULAR_SPEED: f32 = 2.2;
const STARFIELD_RADIUS: f32 = 400.0;
const STARFIELD_PATH: &str = "sky/Starfield.jpg";

const ORBIT_COLOR: Color = Color::srgb(0.67, 0.69, 0.78);

struct MoonSpec {
    semi_major_km: f32,
    radius_km: f32,
    period_days: f32,
    color: Color,
    phase: f32,
}

const MOONS: [MoonSpec; 4] = [
    MoonSpec {
        semi_major_km: 421_700.0,
        radius_km: 1_821.6,
        period_days: 1.769,
        color: Color::srgb(0.886, 0.780, 0.353),
        phase: 0.3,
    },
    MoonSpec {
        semi_major_km: 671_034.0,
        radius_km: 1_560.8,
        period_days: 3.551,
        color: Color::srgb(0.910, 0.878, 0.816),
        phase: 2.2,
    },
    MoonSpec {
        semi_major_km: 1_070_412.0,
        radius_km: 2_634.1,
        period_days: 7.155,
        color: Color::srgb(0.639, 0.588, 0.518),
        phase: 4.0,
    },
    MoonSpec {
        semi_major_km: 1_882_709.0,
        radius_km: 2_410.3,
        period_days: 16.689,
        color: Color::srgb(0.435, 0.400, 0.353),
        phase: 5.4,
    },
];

/// Fase da animação de abertura: tocando, revelando a cena principal por trás, ou terminada.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IntroState {
    /// Cena 3D da abertura tocando.
    #[default]
    Playing,
    /// Cena 3D já removida; só o clarão final (UI) ainda cobre a cena principal.
    Revealing,
    /// Abertura terminada, câmera e demais entidades removidas.
    Done,
}

/// Plugin da animação de abertura.
pub struct IntroPlugin;

impl Plugin for IntroPlugin {
    fn build(&self, app: &mut App) {
        let environment = app
            .world()
            .get_resource::<Environment>()
            .cloned()
            .unwrap_or_else(Environment::load);

        let initial = if environment.skip_intro {
            IntroState::Done
        } else {
            IntroState::Playing
        };

        app.insert_resource(environment)
            .insert_state(initial)
            .init_resource::<IntroClock>()
            .init_resource::<IntroFrame>()
            .init_gizmo_group::<IntroGizmos>()
            .add_systems(Startup, configure_gizmos)
            .add_systems(
                OnEnter(IntroState::Playing),
                (spawn_world, overlay::spawn_overlay).chain(),
            )
            .add_systems(
                Update,
                (
                    skip_on_input,
                    tick,
                    animate_world,
                    draw_orbits,
                    overlay::update_overlay,
                    check_transition,
                )
                    .chain()
                    .run_if(in_state(IntroState::Playing)),
            )
            .add_systems(
                Update,
                (tick, overlay::update_overlay, check_done)
                    .chain()
                    .run_if(in_state(IntroState::Revealing)),
            )
            .add_systems(OnEnter(IntroState::Done), despawn_intro);
    }
}

/// Relógio da abertura, em segundos desde o início, compartilhado entre `Playing` e `Revealing`.
#[derive(Resource, Default)]
struct IntroClock {
    t: f32,
}

/// Valores derivados do relógio, lidos por sistemas que não precisam recalculá-los.
#[derive(Resource, Default)]
struct IntroFrame {
    fade: f32,
}

#[derive(Default, Reflect, GizmoConfigGroup)]
struct IntroGizmos;

#[derive(Component)]
struct IntroEntity;

#[derive(Component)]
struct IntroWorld;

#[derive(Component)]
struct IntroCamera;

#[derive(Component)]
struct IntroSun;

#[derive(Component)]
struct Jupiter;

#[derive(Component)]
struct Moon {
    index: usize,
    orbit_radius: f32,
    angular_speed: f32,
    phase: f32,
    radius: f32,
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_out_cubic(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

fn ease_out_back(x: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}

fn ease_in_out_cubic(x: f32) -> f32 {
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
    }
}

fn reveal(t: f32, at: f32) -> f32 {
    ((t - at) / REVEAL_TIME).clamp(0.0, 1.0)
}

fn moon_reveal_time(index: usize) -> f32 {
    0.7 + 0.15 * index as f32
}

fn orbit_radius(spec: &MoonSpec) -> f32 {
    MAX_ORBIT * (spec.semi_major_km / MOONS[3].semi_major_km).sqrt()
}

fn angular_speed(spec: &MoonSpec) -> f32 {
    IO_ANGULAR_SPEED * MOONS[0].period_days / spec.period_days
}

fn moon_radius(spec: &MoonSpec) -> f32 {
    MOON_SCALE * spec.radius_km / MOONS[2].radius_km
}

fn light_direction(t: f32) -> Vec3 {
    let progress = ease_in_out_cubic((t / ZOOM_START).clamp(0.0, 1.0));
    let angle = (205.0 - 80.0 * progress).to_radians();
    Vec3::new(angle.cos(), angle.sin(), 0.5).normalize()
}

fn camera_distance(zoom: f32) -> f32 {
    START_DISTANCE * (END_DISTANCE / START_DISTANCE).powf(ease_in_out_cubic(zoom))
}

fn camera_transform(distance: f32) -> Transform {
    let (sin, cos) = CAMERA_ELEVATION.sin_cos();
    Transform::from_xyz(0.0, distance * sin, distance * cos).looking_at(Vec3::ZERO, Vec3::Y)
}

/// `Sphere::mesh().uv()` do Bevy 0.18 gera os polos no eixo Z local, não Y (ver
/// `bevy_mesh::primitives::dim3::sphere::SphereMeshBuilder::uv`: a latitude anda com o eixo Z,
/// a longitude gira em torno dele). Como a textura de Júpiter assume polo no topo/base da
/// imagem e equador no meio, essa rotação fixa realinha o polo da malha com o eixo Y do mundo,
/// que é o eixo real de rotação de Júpiter e o eixo perpendicular ao plano das órbitas das luas.
fn jupiter_pole_alignment() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

/// Orientação de Júpiter: gira em torno do eixo Y do mundo, já com o polo da malha realinhado.
fn jupiter_rotation(spin_angle: f32) -> Quat {
    Quat::from_rotation_y(spin_angle) * jupiter_pole_alignment()
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<IntroGizmos>();
    config.render_layers = RenderLayers::layer(INTRO_LAYER);
}

fn spawn_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut clock: ResMut<IntroClock>,
) {
    *clock = IntroClock::default();
    let layer = RenderLayers::layer(INTRO_LAYER);

    commands.spawn((
        IntroEntity,
        IntroCamera,
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        camera_transform(START_DISTANCE),
        layer.clone(),
    ));

    commands.spawn((
        IntroEntity,
        IntroWorld,
        IntroSun,
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(light_direction(0.0)).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
    ));

    commands.spawn((
        IntroEntity,
        IntroWorld,
        NotShadowCaster,
        Mesh3d(meshes.add(Sphere::new(STARFIELD_RADIUS).mesh().uv(64, 32))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(STARFIELD_PATH)),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        layer.clone(),
    ));

    commands.spawn((
        IntroEntity,
        IntroWorld,
        Jupiter,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(128, 64))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(texture::jupiter_image())),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            ..default()
        })),
        Transform::from_scale(Vec3::ZERO),
        layer.clone(),
    ));

    let moon_mesh = meshes.add(Sphere::new(1.0).mesh().uv(48, 24));
    for (index, spec) in MOONS.iter().enumerate() {
        commands.spawn((
            IntroEntity,
            IntroWorld,
            Moon {
                index,
                orbit_radius: orbit_radius(spec),
                angular_speed: angular_speed(spec),
                phase: spec.phase,
                radius: moon_radius(spec),
            },
            Mesh3d(moon_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: spec.color,
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_scale(Vec3::ZERO),
            layer.clone(),
        ));
    }
}

fn skip_on_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut clock: ResMut<IntroClock>,
) {
    let pressed = keys.get_just_pressed().next().is_some() || mouse.just_pressed(MouseButton::Left);
    if pressed && clock.t < SKIP_TARGET {
        clock.t = SKIP_TARGET;
    }
}

fn tick(time: Res<Time>, mut clock: ResMut<IntroClock>) {
    clock.t += time.delta_secs().min(MAX_FRAME_DELTA);
}

#[allow(clippy::type_complexity)]
fn animate_world(
    clock: Res<IntroClock>,
    mut frame: ResMut<IntroFrame>,
    mut camera: Query<&mut Transform, With<IntroCamera>>,
    mut sun: Query<&mut Transform, (With<IntroSun>, Without<IntroCamera>)>,
    mut jupiter: Query<&mut Transform, (With<Jupiter>, Without<IntroCamera>, Without<IntroSun>)>,
    mut moons: Query<
        (&Moon, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        (Without<IntroCamera>, Without<IntroSun>, Without<Jupiter>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = clock.t;
    let zoom = ((t - ZOOM_START) / (ZOOM_END - ZOOM_START)).clamp(0.0, 1.0);
    let fade = 1.0 - smoothstep(0.05, 0.3, zoom);
    let light = light_direction(t);
    let jupiter_scale = ease_out_back(reveal(t, 0.0));

    frame.fade = fade;

    if let Ok(mut transform) = camera.single_mut() {
        *transform = camera_transform(camera_distance(zoom));
    }

    if let Ok(mut transform) = sun.single_mut() {
        *transform = Transform::from_translation(light).looking_at(Vec3::ZERO, Vec3::Y);
    }

    if let Ok(mut transform) = jupiter.single_mut() {
        transform.scale = Vec3::new(
            jupiter_scale,
            jupiter_scale,
            jupiter_scale * JUPITER_OBLATENESS,
        );
        transform.rotation = jupiter_rotation(t * JUPITER_SPIN);
    }

    for (moon, mut transform, material) in &mut moons {
        let angle = moon.phase + moon.angular_speed * t;
        let (sin, cos) = angle.sin_cos();
        let grow = ease_out_cubic(reveal(t, moon_reveal_time(moon.index)));
        transform.translation = Vec3::new(cos * moon.orbit_radius, 0.0, -sin * moon.orbit_radius);
        transform.scale = Vec3::splat(moon.radius * grow);

        if fade < 1.0 && let Some(material) = materials.get_mut(&material.0) {
            material.alpha_mode = AlphaMode::Blend;
            material.base_color.set_alpha(fade);
        }
    }
}

fn draw_orbits(clock: Res<IntroClock>, frame: Res<IntroFrame>, mut gizmos: Gizmos<IntroGizmos>) {
    let plane = Isometry3d::from_rotation(Quat::from_rotation_x(FRAC_PI_2));
    for (index, spec) in MOONS.iter().enumerate() {
        let grow = ease_out_cubic(reveal(clock.t, 0.35 + 0.15 * index as f32));
        let alpha = 0.26 * grow * frame.fade;
        if alpha <= 0.0 {
            continue;
        }
        let radius = orbit_radius(spec) * (0.6 + 0.4 * grow);
        gizmos
            .circle(plane, radius, ORBIT_COLOR.with_alpha(alpha))
            .resolution(128);
    }
}

/// Ao cruzar o instante de transição, remove a cena 3D da abertura, limpa o clear color da
/// câmera (a UI por cima fica responsável pelo clarão final) e passa para `Revealing`.
fn check_transition(
    mut commands: Commands,
    clock: Res<IntroClock>,
    world: Query<Entity, With<IntroWorld>>,
    mut camera: Query<&mut Camera, With<IntroCamera>>,
    mut next_state: ResMut<NextState<IntroState>>,
) {
    if clock.t < TRANSITION {
        return;
    }

    for entity in &world {
        commands.entity(entity).despawn();
    }
    if let Ok(mut camera) = camera.single_mut() {
        camera.clear_color = ClearColorConfig::None;
    }
    next_state.set(IntroState::Revealing);
}

fn check_done(clock: Res<IntroClock>, mut next_state: ResMut<NextState<IntroState>>) {
    if clock.t >= DURATION {
        next_state.set(IntroState::Done);
    }
}

fn despawn_intro(mut commands: Commands, entities: Query<Entity, With<IntroEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbits_grow_outwards_and_end_at_max_orbit() {
        let radii: Vec<f32> = MOONS.iter().map(orbit_radius).collect();
        assert!(radii.windows(2).all(|pair| pair[0] < pair[1]));
        assert!((radii[3] - MAX_ORBIT).abs() < 1e-4);
        assert!(radii[0] > 1.0);
    }

    #[test]
    fn inner_moons_orbit_faster() {
        let speeds: Vec<f32> = MOONS.iter().map(angular_speed).collect();
        assert!(speeds.windows(2).all(|pair| pair[0] > pair[1]));
    }

    #[test]
    fn easings_start_at_zero_and_end_at_one() {
        for easing in [ease_out_cubic, ease_out_back, ease_in_out_cubic] {
            assert!(easing(0.0).abs() < 1e-5);
            assert!((easing(1.0) - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn light_starts_on_the_left_and_is_normalized() {
        let light = light_direction(0.0);
        assert!(light.x < 0.0);
        assert!((light.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn camera_travels_from_start_to_end_distance() {
        assert!((camera_distance(0.0) - START_DISTANCE).abs() < 1e-4);
        assert!((camera_distance(1.0) - END_DISTANCE).abs() < 1e-4);
        assert!(camera_distance(0.5) < START_DISTANCE);
    }

    #[test]
    fn transition_happens_inside_the_zoom() {
        const { assert!(TRANSITION > ZOOM_START && TRANSITION < ZOOM_END) };
        const { assert!(DURATION > TRANSITION) };
        const { assert!(SKIP_TARGET < ZOOM_START) };
    }

    #[test]
    fn jupiter_rotation_keeps_the_mesh_pole_aligned_with_world_y() {
        for spin in [0.0, 0.7, 2.1, 5.0] {
            let world_pole = jupiter_rotation(spin) * Vec3::Z;
            assert!(
                (world_pole - Vec3::Y).length() < 1e-5,
                "polo da malha não ficou no eixo Y para spin={spin}: {world_pole:?}"
            );
        }
    }

    #[test]
    fn jupiter_rotation_sweeps_the_equator_around_world_y() {
        let start = jupiter_rotation(0.0) * Vec3::X;
        let quarter_turn = jupiter_rotation(FRAC_PI_2) * Vec3::X;
        assert!(start.y.abs() < 1e-5, "ponto do equador saiu do plano XZ");
        assert!(
            quarter_turn.y.abs() < 1e-5,
            "ponto do equador saiu do plano XZ"
        );
        assert!((start - quarter_turn).length() > 1.0);
    }

    #[test]
    fn flattening_the_mesh_pole_axis_squishes_world_y() {
        let scaled_pole = jupiter_rotation(1.3) * (Vec3::Z * JUPITER_OBLATENESS);
        assert!((scaled_pole - Vec3::Y * JUPITER_OBLATENESS).length() < 1e-5);
    }

    #[test]
    fn great_red_spot_longitude_faces_the_camera_by_the_end_of_the_intro() {
        // A Grande Mancha Vermelha fica na coluna central da textura (longitude π), que
        // corresponde ao sector central da malha UV e, no equador, ao ponto local (-1, 0, 0)
        // antes de aplicar a rotação de Júpiter.
        let great_red_spot_local_point = Vec3::NEG_X;
        let world_point = jupiter_rotation(DURATION * JUPITER_SPIN) * great_red_spot_local_point;
        // A câmera fica sempre do lado +Z, olhando para a origem (ver `camera_transform`).
        assert!(
            world_point.z > 0.0,
            "Grande Mancha Vermelha não virou para o lado da câmera até o fim da abertura: {world_point:?}"
        );
    }
}
