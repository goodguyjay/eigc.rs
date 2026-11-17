//! Comportamento do limite da área jogável com o plugin real: a câmera a pé nunca passa do raio
//! jogável de Europa e desliza pela borda, e o voo livre de debug fica solto.

use bevy::MinimalPlugins;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::window::CursorOptions;
use eigc_moons::AppState;
use eigc_scene::camera::first_person::FirstPersonCamera;
use eigc_scene::camera::look::CameraLook;
use eigc_scene::camera::mode::MovementMode;
use eigc_scene::camera::plugin::CameraPlugin;
use eigc_terrain::recipe::playable_radius_m;
use std::f32::consts::FRAC_PI_2;
use std::time::Duration;

/// App headless já em `Running`, com a câmera spawnada e relógio de 100 ms por quadro.
fn running_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(InputPlugin)
        .add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )))
        .add_plugins(CameraPlugin);
    app.world_mut().spawn(CursorOptions::default());
    app.update();

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Running);
    for _ in 0..3 {
        app.update();
    }
    app
}

fn camera_position(app: &mut App) -> Vec3 {
    app.world_mut()
        .query_filtered::<&Transform, With<FirstPersonCamera>>()
        .single(app.world())
        .expect("deveria existir exatamente uma câmera")
        .translation
}

fn place_camera(app: &mut App, position: Vec3) {
    app.world_mut()
        .query_filtered::<&mut Transform, With<FirstPersonCamera>>()
        .single_mut(app.world_mut())
        .expect("deveria existir exatamente uma câmera")
        .translation = position;
}

/// Aponta a câmera para +X: com yaw de -90 graus, "frente" é +X e "direita" é +Z.
fn face_positive_x(app: &mut App) {
    app.world_mut()
        .query::<&mut CameraLook>()
        .single_mut(app.world_mut())
        .expect("deveria existir exatamente uma câmera")
        .yaw = -FRAC_PI_2;
}

fn horizontal_distance(position: Vec3) -> f32 {
    position.x.hypot(position.z)
}

/// Testa que uma câmera a pé fora do raio jogável é puxada para dentro, na mesma direção.
#[test]
fn walking_camera_outside_the_playable_radius_is_pulled_back_inside() {
    let radius = playable_radius_m();
    let mut app = running_app();
    place_camera(&mut app, Vec3::new(radius * 1.5, 0.0, -radius * 0.5));

    app.update();

    let position = camera_position(&mut app);
    assert!(
        horizontal_distance(position) <= radius * (1.0 + 1e-5),
        "a câmera ficou fora do raio: {position:?}"
    );
    assert!(
        position.x > 0.0 && position.z < 0.0,
        "a direção deveria ser a mesma: {position:?}"
    );
}

/// Testa que andar para fora da borda não passa do raio jogável, mesmo segurando a tecla por
/// muitos quadros.
#[test]
fn walking_toward_the_edge_never_passes_the_playable_radius() {
    let radius = playable_radius_m();
    let mut app = running_app();
    place_camera(&mut app, Vec3::new(radius - 0.1, 0.0, 0.0));
    face_positive_x(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);

    for _ in 0..30 {
        app.update();
        let position = camera_position(&mut app);
        assert!(
            horizontal_distance(position) <= radius * (1.0 + 1e-5),
            "passou do raio jogável: {position:?}"
        );
    }
}

/// Testa que, andando na diagonal contra a borda, o jogador desliza por ela: o componente que
/// aponta para fora é cortado e o que corre ao longo da borda continua.
#[test]
fn walking_diagonally_against_the_edge_slides_along_it() {
    let radius = playable_radius_m();
    let mut app = running_app();
    place_camera(&mut app, Vec3::new(radius - 0.1, 0.0, 0.0));
    face_positive_x(&mut app);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
    }

    for _ in 0..30 {
        app.update();
    }

    let position = camera_position(&mut app);
    assert!(horizontal_distance(position) <= radius * (1.0 + 1e-5));
    assert!(
        position.z > 1.0,
        "o jogador deveria ter deslizado ao longo da borda: {position:?}"
    );
}

/// Testa que o voo livre de debug não é limitado: a câmera fora do raio fica onde está.
#[test]
fn free_fly_camera_is_not_limited_by_the_playable_radius() {
    let radius = playable_radius_m();
    let mut app = running_app();
    app.insert_resource(MovementMode::FreeFly);
    let outside = Vec3::new(radius * 2.0, 50.0, radius * 0.5);
    place_camera(&mut app, outside);

    app.update();

    assert_eq!(camera_position(&mut app), outside);
}
