//! Cena 3D do menu: câmera, iluminação, fundo estrelado e as quatro luas em fileira, girando
//! lentamente.

use crate::interaction::attach_moon_observers;
use crate::layout::{moon_position, moon_uniform_scale, overview_camera_position};
use bevy::camera::ClearColorConfig;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::{
    AlphaMode, AmbientLight, AssetServer, Assets, Camera, Camera3d, Color, Commands, Component,
    DirectionalLight, Mesh, Mesh3d, MeshMaterial3d, Meshable, Name, Pickable, Query, Res, ResMut,
    Sphere, StandardMaterial, Time, Transform, Vec3, With, default,
};
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnEnter;
use eigc_moons::{AppState, MOON_DISPLAY_ORDER, MoonId};

/// Velocidade de rotação própria das luas no menu, em radianos por segundo.
const MOON_SPIN_SPEED: f32 = 0.15;

/// Raio do domo de estrelas do menu. Fica abaixo do `far` padrão da câmera (1000).
const STAR_DOME_RADIUS: f32 = 500.0;

/// Brilho do domo de estrelas do menu, para o fundo não competir com as luas.
const STAR_DOME_BRIGHTNESS: f32 = 0.6;

/// Marca a câmera do menu.
#[derive(Component, Debug)]
pub(crate) struct MenuCamera;

/// Marca a entidade raiz do modelo 3D de uma lua exibida no menu.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct MenuMoon(pub MoonId);

/// Caminho do modelo `.glb` da lua, relativo à raiz de assets.
fn glb_asset_path(moon_id: MoonId) -> &'static str {
    match moon_id {
        MoonId::Europa => "moons/textures/europa_3d_texture.glb",
        MoonId::Io => "moons/textures/io_3d_texture.glb",
        MoonId::Ganymede => "moons/textures/ganymede_3d_texture.glb",
        MoonId::Callisto => "moons/textures/callisto_3d_texture.glb",
    }
}

/// Monta a cena do menu ao entrar em `MainMenu`. Tudo é removido ao entrar em `Running`, de modo
/// que a câmera e as luas continuam visíveis durante `LoadingMoonProfile`.
pub(crate) fn spawn_menu_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("MenuCamera"),
        MenuCamera,
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.008, 0.012, 0.025)),
            ..default()
        },
        AmbientLight {
            color: Color::WHITE,
            brightness: 60.0,
            ..default()
        },
        Transform::from_translation(overview_camera_position()),
        DespawnOnEnter(AppState::Running),
    ));

    commands.spawn((
        Name::new("MenuKeyLight"),
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(-6.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        DespawnOnEnter(AppState::Running),
    ));

    commands.spawn((
        Name::new("MenuStarDome"),
        Mesh3d(meshes.add(Mesh::from(Sphere::new(1.0).mesh().uv(64, 32)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::linear_rgb(
                STAR_DOME_BRIGHTNESS,
                STAR_DOME_BRIGHTNESS,
                STAR_DOME_BRIGHTNESS,
            ),
            base_color_texture: Some(asset_server.load("sky/Starfield.jpg")),
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(STAR_DOME_RADIUS)),
        NoFrustumCulling,
        Pickable::IGNORE,
        DespawnOnEnter(AppState::Running),
    ));

    for moon_id in MOON_DISPLAY_ORDER {
        let mut moon = commands.spawn((
            Name::new(format!("MenuMoon {moon_id:?}")),
            MenuMoon(moon_id),
            SceneRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(glb_asset_path(moon_id))),
            ),
            Transform::from_translation(moon_position(moon_id))
                .with_scale(Vec3::splat(moon_uniform_scale(moon_id))),
            DespawnOnEnter(AppState::Running),
        ));
        attach_moon_observers(&mut moon, moon_id);
    }
}

/// Gira lentamente cada lua do menu em torno do próprio eixo vertical.
pub(crate) fn spin_menu_moons(time: Res<Time>, mut moons: Query<&mut Transform, With<MenuMoon>>) {
    for mut transform in &mut moons {
        transform.rotate_y(MOON_SPIN_SPEED * time.delta_secs());
    }
}
