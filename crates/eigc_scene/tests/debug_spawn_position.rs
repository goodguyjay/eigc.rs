//! A posição de spawn de debug (`--spawn`) só existe com a feature `dev`: a câmera nasce onde foi
//! pedido e em voo livre, senão `follow_ground` descartaria o `y`.
#![cfg(feature = "dev")]

use bevy::MinimalPlugins;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::CursorOptions;
use eigc_moons::AppState;
use eigc_scene::camera::mode::MovementMode;
use eigc_scene::camera::plugin::{CameraPlugin, DebugSpawnPosition};

/// App headless com o plugin de câmera, já em `Running`, e uma entidade que faz o papel da janela.
fn running_app(debug_spawn: Option<Vec3>) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(InputPlugin)
        .add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_plugins(CameraPlugin);
    app.world_mut().spawn(CursorOptions::default());
    if let Some(position) = debug_spawn {
        app.insert_resource(DebugSpawnPosition(position));
    }
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
        .query_filtered::<&Transform, With<Camera3d>>()
        .single(app.world())
        .expect("deveria existir exatamente uma câmera")
        .translation
}

/// Testa que, com `DebugSpawnPosition`, a câmera nasce na posição pedida (inclusive o `y`) e o
/// modo de movimento passa a ser voo livre.
#[test]
fn camera_spawns_at_the_requested_position_in_free_fly() {
    let requested = Vec3::new(1200.0, 80.0, -3400.0);
    let mut app = running_app(Some(requested));

    assert_eq!(camera_position(&mut app), requested);
    assert_eq!(
        *app.world().resource::<MovementMode>(),
        MovementMode::FreeFly
    );
}

/// Testa que sem `DebugSpawnPosition` a câmera nasce na clareira de spawn, em primeira pessoa.
#[test]
fn camera_spawns_in_the_clearing_in_walk_mode_without_a_debug_position() {
    let mut app = running_app(None);

    let position = camera_position(&mut app);
    assert_eq!((position.x, position.z), (0.0, 0.0));
    assert_eq!(*app.world().resource::<MovementMode>(), MovementMode::Walk);
}
