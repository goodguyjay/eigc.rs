//! Estado de alto nível da aplicação, carregando o perfil da lua ativa ou rodando com o terreno
//! já spawnado.

use crate::moon_loading::{start_loading_active_moon_profile, transition_when_moon_profile_loaded};
use crate::scene_placeholder::spawn_placeholder_light;
use bevy::DefaultPlugins;
use bevy::prelude::{App, AppExtStates, AssetPlugin, PluginGroup, Startup, Update, default};
use eigc_moons::{AppState, MoonPlugin};
use eigc_scene::camera::free_fly_camera::FreeFlyCameraPlugin;
use eigc_scene::sky::SkyPlugin;
use eigc_sim::TimeFlowPlugin;
use eigc_terrain::pipeline::TerrainPlugin;

pub mod moon_loading;
mod scene_placeholder;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "../../assets".to_string(),
            ..default()
        }))
        .init_state::<AppState>()
        .add_plugins(MoonPlugin)
        .add_plugins(TerrainPlugin)
        .add_plugins(TimeFlowPlugin)
        .add_plugins(FreeFlyCameraPlugin)
        .add_plugins(SkyPlugin)
        .add_systems(
            Startup,
            (
                start_loading_active_moon_profile,
                spawn_placeholder_light,
            ),
        )
        .add_systems(Update, transition_when_moon_profile_loaded)
        .run();
}
