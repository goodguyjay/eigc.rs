//! Estado de alto nível da aplicação: menu de seleção de lua, carregamento do perfil da lua
//! escolhida e simulação rodando com o terreno já spawnado.

use crate::moon_loading::transition_when_moon_profile_loaded;
use bevy::DefaultPlugins;
#[cfg(feature = "dev")]
use bevy::prelude::Vec3;
use bevy::prelude::{
    App, AppExtStates, AssetPlugin, IntoScheduleConfigs, MessageWriter, NextState, OnEnter, OnExit,
    PluginGroup, Res, ResMut, Startup, State, SystemCondition, Update, default, in_state,
    resource_exists,
};
#[cfg(feature = "dev")]
use eigc_menu::ExploreRequested;
use eigc_menu::{MenuPlugin, ShowMenu};
#[cfg(feature = "dev")]
use eigc_moons::MoonId;
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonPlugin};
use eigc_scene::camera::plugin::CameraPlugin;
#[cfg(feature = "dev")]
use eigc_scene::camera::plugin::DebugSpawnPosition;
use eigc_scene::intro::{IntroPlugin, IntroState};
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

/// Libera o menu quando a abertura sai de `Playing`, seja por ter tocado até o fim ou por ter
/// sido pulada no meio.
fn show_menu_when_intro_leaves_playing(mut show_menu: MessageWriter<ShowMenu>) {
    show_menu.write(ShowMenu);
}

/// Cobre o caso de a abertura já nascer em `Done` (flag `skip_intro`), quando `OnExit(Playing)`
/// nunca dispara porque o estado nunca chega a entrar em `Playing`.
fn show_menu_if_intro_already_skipped(
    intro_state: Res<State<IntroState>>,
    show_menu: MessageWriter<ShowMenu>,
) {
    if *intro_state.get() != IntroState::Playing {
        show_menu_when_intro_leaves_playing(show_menu);
    }
}

/// Força o fim da abertura assim que o jogador escolhe uma lua, mesmo que o clarão final ainda
/// esteja se desfazendo. Sem isso, a câmera da abertura podia continuar viva ao mesmo tempo que a
/// câmera do jogo nasce em `Running`, e sistemas do céu que buscam "a" câmera via
/// `Query<_, With<Camera3d>>::single()` ficavam confusos entre as duas (ou, pior, usavam a da
/// abertura sozinha enquanto a do jogo ainda não existia).
fn finish_intro_when_leaving_menu(mut next_intro_state: ResMut<NextState<IntroState>>) {
    next_intro_state.set(IntroState::Done);
}

/// Atalho de debug (`--europa`): pede a exploração de Europa.
#[cfg(feature = "dev")]
fn send_europa_request(mut requests: MessageWriter<ExploreRequested>) {
    requests.write(ExploreRequested(MoonId::Europa));
}

/// Cobre o caso de a abertura já nascer em `Done` (flag `skip_intro`), quando `OnEnter(Done)`
/// nunca dispara porque o estado nunca chega a *entrar* em `Done` (ele já começa lá).
#[cfg(feature = "dev")]
fn send_europa_request_if_intro_already_done(
    intro_state: Res<State<IntroState>>,
    requests: MessageWriter<ExploreRequested>,
) {
    if *intro_state.get() == IntroState::Done {
        send_europa_request(requests);
    }
}

/// Lê `--spawn x y z` dos argumentos de linha de comando. `Ok(None)` quando a flag não aparece;
/// `Err` com a mensagem de uso quando faltam coordenadas ou alguma não é número finito.
#[cfg(feature = "dev")]
fn parse_spawn_position(args: &[String]) -> Result<Option<Vec3>, String> {
    const USAGE: &str = "uso: --spawn <x> <y> <z> (números finitos, em metros)";

    let Some(flag_index) = args.iter().position(|arg| arg == "--spawn") else {
        return Ok(None);
    };

    let coordinates: Vec<f32> = args[flag_index + 1..]
        .iter()
        .take(3)
        .map_while(|arg| arg.parse::<f32>().ok().filter(|value| value.is_finite()))
        .collect();

    match coordinates[..] {
        [x, y, z] => Ok(Some(Vec3::new(x, y, z))),
        _ => Err(USAGE.to_string()),
    }
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
    .add_plugins(CameraPlugin)
    .add_plugins(SkyPlugin)
    .add_plugins(IntroPlugin)
    .add_plugins(MenuPlugin)
    .add_systems(Startup, show_menu_if_intro_already_skipped)
    .add_systems(
        OnExit(IntroState::Playing),
        show_menu_when_intro_leaves_playing,
    )
    .add_systems(
        OnEnter(AppState::LoadingMoonProfile),
        finish_intro_when_leaving_menu,
    );

    gate_simulation_clock(&mut app).add_systems(
        Update,
        transition_when_moon_profile_loaded.run_if(
            in_state(AppState::LoadingMoonProfile).and(resource_exists::<ActiveMoonProfileHandle>),
        ),
    );

    #[cfg(feature = "dev")]
    app.add_plugins(eigc_perf::debug_tools::DebugToolsPlugin);

    // Só debug: espera a abertura terminar de vez (clarão desfeito) antes de pedir a exploração
    // de Europa, para a transição do menu pro jogo não ficar brusca.
    #[cfg(feature = "dev")]
    if std::env::args().any(|arg| arg == "--europa") {
        app.add_systems(Startup, send_europa_request_if_intro_already_done)
            .add_systems(OnEnter(IntroState::Done), send_europa_request);
    }

    // Só debug: `--spawn x y z` (usado junto de `--europa`) faz a câmera nascer na posição pedida.
    #[cfg(feature = "dev")]
    {
        let args: Vec<String> = std::env::args().collect();
        match parse_spawn_position(&args) {
            Ok(Some(position)) => {
                app.insert_resource(DebugSpawnPosition(position));
            }
            Ok(None) => {}
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }

    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::input::InputPlugin;
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

    #[cfg(feature = "dev")]
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// Testa que `--spawn` lê três coordenadas, inclusive negativas e fracionárias.
    #[cfg(feature = "dev")]
    #[test]
    fn spawn_flag_reads_three_coordinates() {
        let parsed = parse_spawn_position(&args(&[
            "eigc_app", "--europa", "--spawn", "1200", "-35.5", "-3400",
        ]));

        assert_eq!(parsed, Ok(Some(Vec3::new(1200.0, -35.5, -3400.0))));
    }

    /// Testa que sem `--spawn` não há posição forçada.
    #[cfg(feature = "dev")]
    #[test]
    fn missing_spawn_flag_means_no_forced_position() {
        assert_eq!(
            parse_spawn_position(&args(&["eigc_app", "--europa"])),
            Ok(None)
        );
    }

    /// Testa que coordenadas faltando, não numéricas ou não finitas são recusadas, em vez de
    /// virar um spawn em lugar nenhum.
    #[cfg(feature = "dev")]
    #[test]
    fn spawn_flag_rejects_missing_or_invalid_coordinates() {
        for invalid in [
            args(&["--spawn"]),
            args(&["--spawn", "1", "2"]),
            args(&["--spawn", "1", "dois", "3"]),
            args(&["--spawn", "1", "NaN", "3"]),
            args(&["--spawn", "1", "inf", "3"]),
        ] {
            assert!(
                parse_spawn_position(&invalid).is_err(),
                "aceitou {invalid:?}"
            );
        }
    }

    /// Testa que o relógio da simulação (`SimTime`) não avança em `MainMenu`
    /// nem em `LoadingMoonProfile`, e só avança em `Running`. Sem o gate, o
    /// tempo acumulava durante o menu e o céu começava numa fase arbitrária.
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
