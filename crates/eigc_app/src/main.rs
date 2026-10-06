//! Estado de alto nível da aplicação: menu de seleção de lua, carregamento do perfil da lua
//! escolhida e simulação rodando com o terreno já spawnado.

use crate::moon_loading::transition_when_moon_profile_loaded;
use bevy::DefaultPlugins;
use bevy::prelude::{
    App, AppExtStates, AssetPlugin, IntoScheduleConfigs, PluginGroup, SystemCondition, Update,
    default, in_state, resource_exists,
};
use eigc_menu::MenuPlugin;
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonPlugin};
use eigc_scene::camera::free_fly_camera::FreeFlyCameraPlugin;
use eigc_scene::sky::SkyPlugin;
use eigc_sim::{SimSet, TimeFlowPlugin};
use eigc_terrain::pipeline::TerrainPlugin;

/// Sistemas de transição de estado ao concluir o carregamento do perfil da lua escolhida.
pub mod moon_loading;

/// Faz o relógio da simulação avançar só em `Running`, para o céu não começar numa fase
/// arbitrária depois do tempo gasto no menu e no carregamento.
fn gate_simulation_clock(app: &mut App) -> &mut App {
    app.configure_sets(Update, SimSet::Advance.run_if(in_state(AppState::Running)))
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        file_path: "../../assets".to_string(),
        ..default()
    }))
    .init_state::<AppState>()
    .add_plugins(MoonPlugin)
    .add_plugins(TerrainPlugin)
    .add_plugins(TimeFlowPlugin)
    .add_plugins(FreeFlyCameraPlugin)
    .add_plugins(SkyPlugin)
    .add_plugins(MenuPlugin);

    gate_simulation_clock(&mut app).add_systems(
        Update,
        transition_when_moon_profile_loaded.run_if(
            in_state(AppState::LoadingMoonProfile).and(resource_exists::<ActiveMoonProfileHandle>),
        ),
    );

    #[cfg(feature = "dev")]
    app.add_plugins(eigc_perf::debug_tools::DebugToolsPlugin);

    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::input::InputPlugin;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use eigc_sim::SimTime;
    use std::time::Duration;

    /// App headless com o relógio de simulação e o gate de estado, com 100 ms por quadro.
    fn clock_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(InputPlugin)
            .add_plugins(StatesPlugin)
            .init_state::<AppState>()
            .add_plugins(TimeFlowPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )));
        gate_simulation_clock(&mut app);
        app
    }

    fn run_frames(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
        }
    }

    /// Sem o gate, `SimTime` acumulava durante o menu e o céu começava numa fase arbitrária.
    #[test]
    fn sim_time_only_advances_while_running() {
        let mut app = clock_app();

        run_frames(&mut app, 5);
        assert_eq!(
            app.world().resource::<SimTime>().0,
            0.0,
            "avançou em MainMenu"
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::LoadingMoonProfile);
        run_frames(&mut app, 5);
        assert_eq!(
            app.world().resource::<SimTime>().0,
            0.0,
            "avançou em LoadingMoonProfile"
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Running);
        run_frames(&mut app, 5);
        assert!(
            app.world().resource::<SimTime>().0 > 0.0,
            "não avançou em Running"
        );
    }
}
