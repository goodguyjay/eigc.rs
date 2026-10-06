//! Plugin que registra a cena 3D do menu, a interação com as luas, a UI e o tratamento de
//! pedidos de exploração.

use crate::explore::{ExploreRequested, NotImplementedNotice, handle_explore_requests};
use crate::interaction::{
    MenuSelection, animate_menu_camera, animate_moon_scale, exit_focus_on_escape,
};
use crate::scene::{spawn_menu_scene, spin_menu_moons};
use crate::ui::{
    ToastState, highlight_labels, position_moon_labels, refresh_panel_content, slide_detail_panel,
    spawn_loading_overlay, spawn_menu_ui, style_buttons, update_overview_visibility, update_toast,
};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::{App, IntoScheduleConfigs, OnEnter, Plugin, Update, in_state};
use eigc_moons::AppState;

/// Registra o menu de seleção de lua, ativo enquanto a aplicação está em `AppState::MainMenu`.
/// Em `LoadingMoonProfile`, mantém a cena do menu e exibe a tela de carregamento por cima.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MeshPickingPlugin)
            .add_message::<ExploreRequested>()
            .add_message::<NotImplementedNotice>()
            .init_resource::<MenuSelection>()
            .init_resource::<ToastState>()
            .add_systems(
                OnEnter(AppState::MainMenu),
                (spawn_menu_scene, spawn_menu_ui),
            )
            .add_systems(OnEnter(AppState::LoadingMoonProfile), spawn_loading_overlay)
            .add_systems(
                Update,
                (
                    handle_explore_requests,
                    exit_focus_on_escape,
                    animate_menu_camera,
                    animate_moon_scale,
                    spin_menu_moons,
                    refresh_panel_content,
                    slide_detail_panel,
                    update_overview_visibility,
                    position_moon_labels,
                    highlight_labels,
                    style_buttons,
                    update_toast,
                )
                    .chain()
                    .run_if(in_state(AppState::MainMenu)),
            );
    }
}
