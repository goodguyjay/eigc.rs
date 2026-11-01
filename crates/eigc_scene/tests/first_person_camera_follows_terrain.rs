//! Comportamento da câmera em primeira pessoa com o plugin real: o olho acompanha a altura do
//! terreno enquanto anda, e só faz isso no modo `Walk` e com o terreno já carregado.

use bevy::MinimalPlugins;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::window::CursorOptions;
use eigc_moons::AppState;
use eigc_scene::camera::first_person::FirstPersonCamera;
use eigc_scene::camera::mode::MovementMode;
use eigc_scene::camera::plugin::CameraPlugin;
use eigc_terrain::height::{HeightSource, arc};
use eigc_terrain::pipeline::HeightResource;
use std::time::Duration;

/// Terreno inclinado só para teste: base de 5 m e sobe 0,1 m por metro andado em -Z.
struct Ramp;
impl HeightSource for Ramp {
    fn height_at(&self, _x: f32, z: f32) -> f32 {
        5.0 - 0.1 * z
    }
}

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

fn eye_position(app: &mut App) -> Vec3 {
    app.world_mut()
        .query_filtered::<&Transform, With<FirstPersonCamera>>()
        .single(app.world())
        .expect("deveria existir exatamente uma câmera")
        .translation
}

fn hold_forward(app: &mut App, frames: usize) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..frames {
        app.update();
    }
}

/// Testa que, ao andar para frente em terreno que sobe, o olho continua a 1,75 m do chão do
/// ponto em que está, e que a câmera de fato saiu do lugar.
#[test]
fn walking_keeps_the_eye_at_eye_height_above_rising_terrain() {
    let mut app = running_app();
    app.insert_resource(HeightResource(arc(Ramp)));
    app.update();
    let eye_height = FirstPersonCamera::default().eye_height;
    assert_eq!(eye_position(&mut app).y, 5.0 + eye_height);

    hold_forward(&mut app, 10);

    let position = eye_position(&mut app);
    assert!(position.z < -1.0, "a câmera não andou: {position:?}");
    let expected_y = 5.0 - 0.1 * position.z + eye_height;
    assert!(
        (position.y - expected_y).abs() < 1e-4,
        "olho em y = {}, esperado {expected_y}",
        position.y
    );
}

/// Testa que, no modo `FreeFly`, a câmera não é puxada para o terreno.
#[test]
fn free_fly_mode_does_not_snap_the_camera_to_the_terrain() {
    let mut app = running_app();
    app.insert_resource(MovementMode::FreeFly)
        .insert_resource(HeightResource(arc(Ramp)));
    let initial_y = eye_position(&mut app).y;

    app.update();

    assert_eq!(eye_position(&mut app).y, initial_y);
}

/// Testa que, sem `HeightResource` (terreno ainda não montado), a câmera anda sem panic e sem
/// mudar de altura.
#[test]
fn walking_without_terrain_loaded_neither_panics_nor_changes_height() {
    let mut app = running_app();
    let initial_y = eye_position(&mut app).y;

    hold_forward(&mut app, 5);

    assert_eq!(eye_position(&mut app).y, initial_y);
}
