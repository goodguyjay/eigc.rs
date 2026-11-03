//! Estado e cliques do painel de configurações: abrir pela engrenagem, fechar pelo X ou por Esc,
//! e o seletor de tela cheia. Nenhum dos dois é aplicado de verdade ainda: tela cheia não muda a
//! janela, e nada é salvo em disco.

use crate::ui::FullscreenOption;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::{ButtonInput, Click, KeyCode, On, Pointer, Res, ResMut, Resource};

/// Se o painel de configurações está aberto e o valor do seletor de tela cheia.
#[derive(Resource, Default, Debug)]
pub(crate) struct SettingsState {
    /// Painel aberto pela engrenagem, fechado pelo X.
    pub panel_open: bool,
    /// Valor do seletor de tela cheia, só visual por enquanto.
    pub fullscreen_enabled: bool,
}

/// Clique na engrenagem: alterna o painel de configurações aberto/fechado.
pub(crate) fn on_settings_button_clicked(
    click: On<Pointer<Click>>,
    mut settings: ResMut<SettingsState>,
) {
    if click.button == PointerButton::Primary {
        settings.panel_open = !settings.panel_open;
    }
}

/// Clique no X: fecha o painel de configurações.
pub(crate) fn on_close_settings_panel_clicked(
    click: On<Pointer<Click>>,
    mut settings: ResMut<SettingsState>,
) {
    if click.button == PointerButton::Primary {
        settings.panel_open = false;
    }
}

/// Esc fecha o painel de configurações, para o rodapé "Esc para fechar" ser verdade.
pub(crate) fn close_settings_panel_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<SettingsState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) && settings.panel_open {
        settings.panel_open = false;
    }
}

/// Observer de clique numa opção do seletor de tela cheia. Diferente de um alternador, define
/// `fullscreen_enabled` para o valor daquela opção especificamente, por isso `option` é fechado
/// (capturado) na closure retornada em vez de lido de volta da entidade clicada.
pub(crate) fn on_fullscreen_option_clicked(
    option: FullscreenOption,
) -> impl Fn(On<Pointer<Click>>, ResMut<SettingsState>) {
    move |click: On<Pointer<Click>>, mut settings: ResMut<SettingsState>| {
        if click.button == PointerButton::Primary {
            settings.fullscreen_enabled = option == FullscreenOption::Yes;
        }
    }
}
