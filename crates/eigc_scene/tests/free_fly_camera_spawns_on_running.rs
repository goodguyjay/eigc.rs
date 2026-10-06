//! Regressão do menu inicial: a câmera de voo livre só pode existir, e o cursor só pode ser
//! capturado, a partir de `AppState::Running`. Antes disso o menu precisa do cursor livre.

use bevy::MinimalPlugins;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::{CursorGrabMode, CursorOptions};
use eigc_moons::AppState;
use eigc_scene::camera::free_fly_camera::*;

/// App headless com o plugin de câmera e uma entidade que faz o papel da janela.
fn app_with_camera_plugin() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(InputPlugin)
        .add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_plugins(FreeFlyCameraPlugin);
    app.world_mut().spawn(CursorOptions::default());
    app
}

fn camera_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&FreeFlyCamera>()
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

#[test]
fn camera_is_absent_and_cursor_stays_free_before_running() {
    let mut app = app_with_camera_plugin();

    for state in [AppState::MainMenu, AppState::LoadingMoonProfile] {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(camera_count(&mut app), 0, "câmera existe em {state:?}");
        assert_eq!(
            cursor(&mut app),
            (true, CursorGrabMode::None),
            "cursor preso em {state:?}"
        );
    }
}

#[test]
fn camera_spawns_and_captures_the_cursor_on_entering_running() {
    let mut app = app_with_camera_plugin();
    app.update();

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Running);
    for _ in 0..3 {
        app.update();
    }

    assert_eq!(camera_count(&mut app), 1);
    assert_eq!(cursor(&mut app), (false, CursorGrabMode::Locked));
}
