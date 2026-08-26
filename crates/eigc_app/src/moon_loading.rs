//! Sistemas responsáveis por iniciar o carregamento do perfil de lua ativa e transicionar
//! a aplicação para o estado running assim que o asset termina de carregar.

use bevy::prelude::{AssetServer, Assets, Commands, NextState, Res, ResMut};
use eigc_moons::profile::MoonProfile;
use eigc_moons::state::{ActiveMoonProfileHandle, AppState};

/// Dispara o carregamento do perfil da lua ativa assim que a aplicação inicia.
pub fn start_loading_active_moon_profile(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load::<MoonProfile>("moons/europa.ron");
    commands.insert_resource(ActiveMoonProfileHandle(handle));
}

/// Dispara o carregamento do perfil da lua ativa assim que a aplicação
/// inicia.
/// todo: Hoje aponta direto para europa.ron; escolha de lua ativa em runtime (menu de seleção) fica
/// para quando eigc_app tiver um menu de verdade.
pub fn transition_when_moon_profile_loaded(
    active_handle: Res<ActiveMoonProfileHandle>,
    moon_profiles: Res<Assets<MoonProfile>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if moon_profiles.get(&active_handle.0).is_some() {
        next_state.set(AppState::Running);
    }
}
