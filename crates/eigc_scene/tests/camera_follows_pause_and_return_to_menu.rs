//! Regressão do menu de pausa e da volta à seleção de lua: a pausa solta o cursor e congela a
//! câmera; continuar recaptura o cursor; sair de `Running` despawna a câmera e solta o cursor.

use bevy::MinimalPlugins;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::{CursorGrabMode, CursorOptions};
use eigc_moons::{AppState, PauseState};
use eigc_scene::camera::first_person::FirstPersonCamera;
use eigc_scene::camera::plugin::CameraPlugin;

/// App headless com o plugin de câmera, o sub-estado de pausa e uma entidade no papel da janela,
/// já em `Running`.
fn running_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(InputPlugin)
        .add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_sub_state::<PauseState>()
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

fn camera_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&FirstPersonCamera>()
        .iter(app.world())
        .count()
}

fn cursor(app: &mut App) -> (bool, CursorGrabMode) {
    let options = app
        .world_mut()
        .query::<&CursorOptions>()
        .single(app.world())
        .expect("a entidade de janela do teste deveria existir");
    (options.visible, options.grab_mode)
}

fn set_pause(app: &mut App, state: PauseState) {
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(state);
    app.update();
}

fn camera_translation(app: &mut App) -> Vec3 {
    app.world_mut()
        .query_filtered::<&Transform, With<FirstPersonCamera>>()
        .single(app.world())
        .expect("deveria existir uma câmera")
        .translation
}

/// Testa que pausar solta o cursor e que continuar o recaptura.
#[test]
fn pausing_releases_the_cursor_and_resuming_captures_it_again() {
    let mut app = running_app();
    assert_eq!(cursor(&mut app), (false, CursorGrabMode::Locked));

    set_pause(&mut app, PauseState::Paused);
    assert_eq!(cursor(&mut app), (true, CursorGrabMode::None));

    set_pause(&mut app, PauseState::Playing);
    assert_eq!(cursor(&mut app), (false, CursorGrabMode::Locked));
}

/// Testa que, com a pausa aberta, segurar W não move a câmera.
#[test]
fn camera_does_not_move_while_paused() {
    let mut app = running_app();
    set_pause(&mut app, PauseState::Paused);
    let before = camera_translation(&mut app);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..5 {
        app.update();
    }

    assert_eq!(camera_translation(&mut app), before);
}

/// Testa que voltar ao menu a partir da pausa despawna a câmera, solta o cursor e que entrar de
/// novo em `Running` cria exatamente uma câmera.
#[test]
fn returning_to_menu_despawns_the_camera_and_reentering_spawns_only_one() {
    let mut app = running_app();
    set_pause(&mut app, PauseState::Paused);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::MainMenu);
    app.update();

    assert_eq!(
        camera_count(&mut app),
        0,
        "câmera sobreviveu fora de Running"
    );
    assert_eq!(cursor(&mut app), (true, CursorGrabMode::None));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Running);
    app.update();

    assert_eq!(camera_count(&mut app), 1);
    assert_eq!(cursor(&mut app), (false, CursorGrabMode::Locked));
}
