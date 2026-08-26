//! Estado de alto nível da aplicação, carregando o perfil da lua ativa ou rodando com o terreno
//! já spawnado.

use crate::moon_loading::{start_loading_active_moon_profile, transition_when_moon_profile_loaded};
use crate::scene_placeholder::spawn_placeholder_camera_and_light;
use bevy::DefaultPlugins;
use bevy::prelude::{default, App, AppExtStates, AssetPlugin, Startup, Update, PluginGroup};
use eigc_moons::{AppState, MoonPlugin};
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
        .add_systems(Startup, (start_loading_active_moon_profile, spawn_placeholder_camera_and_light),
        )
        .add_systems(Update, transition_when_moon_profile_loaded)
        .run();
}