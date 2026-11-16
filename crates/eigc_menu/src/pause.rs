//! Menu de pausa da exploração: ESC abre e fecha, e os botões continuam, voltam à seleção de lua
//! ou encerram o app. Também prepara o menu de seleção para reaparecer ao sair de `Running`.

use crate::interaction::MenuSelection;
use crate::plugin::ShowMenu;
use crate::settings::SettingsState;
use crate::ui::ToastState;
use bevy::app::AppExit;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::{
    ButtonInput, Click, Commands, KeyCode, MessageWriter, NextState, On, Pointer, Res, ResMut,
    State,
};
use eigc_moons::{ActiveMoonProfileHandle, AppState, PauseState};

/// Estado de pausa depois de apertar ESC: alterna entre jogando e pausado.
pub(crate) fn toggled_pause(current: PauseState) -> PauseState {
    match current {
        PauseState::Playing => PauseState::Paused,
        PauseState::Paused => PauseState::Playing,
    }
}

/// ESC abre o menu de pausa durante a exploração, e fecha se ele já estiver aberto.
pub(crate) fn toggle_pause_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    pause_state: Res<State<PauseState>>,
    mut next_pause_state: ResMut<NextState<PauseState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_pause_state.set(toggled_pause(*pause_state.get()));
    }
}

/// Clique em "Continuar": fecha a pausa e volta à exploração.
pub(crate) fn on_resume_clicked(
    click: On<Pointer<Click>>,
    mut next_pause_state: ResMut<NextState<PauseState>>,
) {
    if click.button == PointerButton::Primary {
        next_pause_state.set(PauseState::Playing);
    }
}

/// Clique em "Voltar à seleção de lua": sai de `Running`. A cena da lua é desmontada pelos
/// próprios plugins (câmera, céu, terreno) ao sair do estado.
pub(crate) fn on_return_to_menu_clicked(
    click: On<Pointer<Click>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    if click.button == PointerButton::Primary {
        next_app_state.set(AppState::MainMenu);
    }
}

/// Clique em "Sair do jogo": encerra o app.
pub(crate) fn on_quit_clicked(click: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>) {
    if click.button == PointerButton::Primary {
        exit.write(AppExit::Success);
    }
}

/// Ao sair de `Running`, solta o perfil da lua explorada e devolve o menu à visão geral, sem foco,
/// aviso ou painel de configurações abertos, e pede para a cena e a UI do menu nascerem de novo.
/// A preferência de tela cheia é mantida: é configuração, não estado da sessão.
pub(crate) fn prepare_menu_return(
    mut commands: Commands,
    mut selection: ResMut<MenuSelection>,
    mut toast: ResMut<ToastState>,
    mut settings: ResMut<SettingsState>,
    mut show_menu: MessageWriter<ShowMenu>,
) {
    commands.remove_resource::<ActiveMoonProfileHandle>();
    *selection = MenuSelection::default();
    *toast = ToastState::default();
    settings.panel_open = false;
    show_menu.write(ShowMenu);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use eigc_moons::MoonId;

    /// Quantos `ShowMenu` foram recebidos durante o teste.
    #[derive(Resource, Default)]
    struct ShowMenuCount(usize);

    fn count_show_menu(mut messages: MessageReader<ShowMenu>, mut count: ResMut<ShowMenuCount>) {
        count.0 += messages.read().count();
    }

    /// Marca uma entidade que deveria sumir junto com o menu de pausa.
    #[derive(Component)]
    struct PauseOnly;

    /// App headless com os estados, o ESC de pausa e o preparo da volta ao menu, já em `Running`.
    fn running_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_state::<AppState>()
            .add_sub_state::<PauseState>()
            .add_message::<ShowMenu>()
            .init_resource::<MenuSelection>()
            .init_resource::<ToastState>()
            .init_resource::<SettingsState>()
            .init_resource::<ShowMenuCount>()
            .add_systems(OnExit(AppState::Running), prepare_menu_return)
            .add_systems(
                Update,
                (
                    toggle_pause_on_escape.run_if(in_state(AppState::Running)),
                    count_show_menu,
                ),
            );
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Running);
        app.update();
        app
    }

    fn pause_state(app: &App) -> Option<PauseState> {
        app.world()
            .get_resource::<State<PauseState>>()
            .map(|state| *state.get())
    }

    /// Aperta ESC por um quadro e roda mais um para a transição de estado ser aplicada. Sem o
    /// `InputPlugin`, ninguém limpa o `just_pressed` sozinho, então o teste solta a tecla à mão.
    fn press_escape(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.release(KeyCode::Escape);
        keyboard.clear();
        app.update();
    }

    /// Testa que ESC alterna nos dois sentidos.
    #[test]
    fn escape_toggles_between_playing_and_paused() {
        assert_eq!(toggled_pause(PauseState::Playing), PauseState::Paused);
        assert_eq!(toggled_pause(PauseState::Paused), PauseState::Playing);
    }

    /// Testa que a exploração começa sem pausa e que ESC abre e fecha a pausa.
    #[test]
    fn escape_opens_and_closes_the_pause_while_running() {
        let mut app = running_app();
        assert_eq!(pause_state(&app), Some(PauseState::Playing));

        press_escape(&mut app);
        assert_eq!(pause_state(&app), Some(PauseState::Paused));

        press_escape(&mut app);
        assert_eq!(pause_state(&app), Some(PauseState::Playing));
    }

    /// Testa que voltar ao menu a partir da pausa remove o estado de pausa, despawna o que é da
    /// pausa, solta o perfil da lua, reseta a seleção e pede o menu de volta uma única vez.
    #[test]
    fn returning_to_menu_from_pause_clears_the_session() {
        let mut app = running_app();
        let handle = Handle::default();
        app.insert_resource(ActiveMoonProfileHandle(handle));
        {
            let mut selection = app.world_mut().resource_mut::<MenuSelection>();
            selection.focused = Some(MoonId::Europa);
            selection.hovered = Some(MoonId::Europa);
        }
        app.world_mut().resource_mut::<SettingsState>().panel_open = true;

        press_escape(&mut app);
        app.world_mut()
            .spawn((PauseOnly, DespawnOnExit(PauseState::Paused)));

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::MainMenu);
        app.update();

        assert_eq!(
            pause_state(&app),
            None,
            "a pausa sobreviveu fora de Running"
        );
        let pause_entities = app
            .world_mut()
            .query::<&PauseOnly>()
            .iter(app.world())
            .count();
        assert_eq!(pause_entities, 0, "a UI de pausa não foi despawnada");
        assert!(!app.world().contains_resource::<ActiveMoonProfileHandle>());
        assert_eq!(
            *app.world().resource::<MenuSelection>(),
            MenuSelection::default()
        );
        assert!(!app.world().resource::<SettingsState>().panel_open);
        assert_eq!(app.world().resource::<ShowMenuCount>().0, 1);
    }
}
