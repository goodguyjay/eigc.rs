//! Pedido de exploração de uma lua e a decisão de iniciar a simulação ou avisar que ainda não
//! está implementada.

use bevy::prelude::{
    AssetServer, Commands, Message, MessageReader, MessageWriter, NextState, Res, ResMut,
};
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonId, MoonInfo, MoonProfile, moon_info};

/// Pedido para explorar a lua indicada, emitido pelo botão "Explorar" do menu.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploreRequested(pub MoonId);

/// Aviso interno de que o jogador tentou usar uma lua que ainda não está implementada. A UI
/// responde com um toast.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotImplementedNotice(pub MoonId);

/// Resultado de avaliar um pedido de exploração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreDecision {
    /// A lua está implementada: carregar o perfil e iniciar a simulação.
    Start(MoonId),
    /// A lua ainda não está implementada: avisar o jogador e permanecer no menu.
    NotImplemented(MoonId),
}

/// Decide o que fazer com um pedido de exploração, com base nos dados de apresentação da lua.
pub(crate) fn decide_explore(info: &MoonInfo) -> ExploreDecision {
    if info.available {
        ExploreDecision::Start(info.moon_id)
    } else {
        ExploreDecision::NotImplemented(info.moon_id)
    }
}

/// Processa os pedidos de exploração: inicia o carregamento do perfil da lua disponível e passa
/// para `LoadingMoonProfile`. Só o primeiro pedido válido do quadro é atendido.
pub(crate) fn handle_explore_requests(
    mut requests: MessageReader<ExploreRequested>,
    mut notices: MessageWriter<NotImplementedNotice>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for request in requests.read() {
        match decide_explore(moon_info(request.0)) {
            ExploreDecision::Start(moon_id) => {
                let handle = asset_server.load::<MoonProfile>(moon_id.profile_asset_path());
                commands.insert_resource(ActiveMoonProfileHandle(handle));
                next_state.set(AppState::LoadingMoonProfile);
                return;
            }
            ExploreDecision::NotImplemented(moon_id) => {
                notices.write(NotImplementedNotice(moon_id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use eigc_moons::{MOON_DISPLAY_ORDER, MoonPlugin};

    /// Avisos de "não implementada" observados durante o teste.
    #[derive(Resource, Default)]
    struct NoticeLog(Vec<MoonId>);

    fn record_notices(
        mut notices: MessageReader<NotImplementedNotice>,
        mut log: ResMut<NoticeLog>,
    ) {
        log.0.extend(notices.read().map(|notice| notice.0));
    }

    /// App headless com só o necessário para o handler: estados, assets de perfil e mensagens.
    fn menu_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin {
                file_path: "../../assets".to_string(),
                ..default()
            })
            .add_plugins(StatesPlugin)
            .add_plugins(MoonPlugin)
            .init_state::<AppState>()
            .add_message::<ExploreRequested>()
            .add_message::<NotImplementedNotice>()
            .init_resource::<NoticeLog>()
            .add_systems(
                Update,
                (handle_explore_requests, record_notices)
                    .chain()
                    .run_if(in_state(AppState::MainMenu)),
            );
        app
    }

    /// Testa que pedir a exploração de Europa insere o handle do perfil, vai
    /// para `LoadingMoonProfile` e não gera aviso de lua não implementada.
    #[test]
    fn exploring_europa_starts_loading_its_profile() {
        let mut app = menu_app();
        app.world_mut()
            .write_message(ExploreRequested(MoonId::Europa));
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::LoadingMoonProfile
        );
        assert!(app.world().contains_resource::<ActiveMoonProfileHandle>());
        assert!(app.world().resource::<NoticeLog>().0.is_empty());
    }

    /// Testa que pedir a exploração de uma lua não implementada (Io) mantém o
    /// estado em `MainMenu`, não carrega perfil e emite só o aviso de não
    /// implementada.
    #[test]
    fn exploring_an_unimplemented_moon_only_raises_a_notice() {
        let mut app = menu_app();
        app.world_mut().write_message(ExploreRequested(MoonId::Io));
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
        assert!(!app.world().contains_resource::<ActiveMoonProfileHandle>());
        assert_eq!(app.world().resource::<NoticeLog>().0, vec![MoonId::Io]);
    }

    /// Testa que a decisão para uma lua disponível (Europa) é iniciar a
    /// exploração.
    #[test]
    fn available_moon_starts_exploration() {
        assert_eq!(
            decide_explore(moon_info(MoonId::Europa)),
            ExploreDecision::Start(MoonId::Europa)
        );
    }

    /// Testa que a decisão para toda lua indisponível é só avisar que ela ainda
    /// não está implementada.
    #[test]
    fn unavailable_moons_are_not_implemented() {
        for moon_id in MOON_DISPLAY_ORDER {
            if moon_id == MoonId::Europa {
                continue;
            }
            assert_eq!(
                decide_explore(moon_info(moon_id)),
                ExploreDecision::NotImplemented(moon_id)
            );
        }
    }
}
