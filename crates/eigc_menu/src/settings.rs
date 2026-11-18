//! Estado e cliques do painel de configurações: abrir pela engrenagem, fechar pelo X ou por Esc,
//! e o seletor de tela cheia, que é aplicado na janela principal. Nada é salvo em disco ainda.

use crate::ui::FullscreenOption;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::{ButtonInput, Click, KeyCode, On, Pointer, Query, Res, ResMut, Resource, With};
use bevy::window::{MonitorSelection, PrimaryWindow, Window, WindowMode};

/// Se o painel de configurações está aberto e o valor do seletor de tela cheia.
#[derive(Resource, Default, Debug)]
pub(crate) struct SettingsState {
    /// Painel aberto pela engrenagem, fechado pelo X.
    pub panel_open: bool,
    /// Valor do seletor de tela cheia. `apply_fullscreen_mode` leva isso para `Window::mode`, num
    /// sentido só: nada além do seletor muda o modo da janela.
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

/// Modo da janela para o valor do seletor de tela cheia. Tela cheia é sem borda (borderless) no
/// monitor atual: não troca a resolução do monitor e não atrapalha o Alt+Tab.
pub(crate) fn window_mode_for(fullscreen_enabled: bool) -> WindowMode {
    if fullscreen_enabled {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    }
}

/// Aplica `SettingsState::fullscreen_enabled` na janela principal. Só escreve quando o modo muda,
/// para não marcar a janela como alterada à toa.
pub(crate) fn apply_fullscreen_mode(
    settings: Res<SettingsState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let mode = window_mode_for(settings.fullscreen_enabled);
    if window.mode != mode {
        window.mode = mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::{App, IntoScheduleConfigs, Update, resource_changed};

    /// App headless com o sistema de tela cheia e uma janela principal criada à mão.
    fn fullscreen_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SettingsState>()
            .add_systems(
                Update,
                apply_fullscreen_mode.run_if(resource_changed::<SettingsState>),
            );
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app
    }

    fn primary_window_mode(app: &mut App) -> WindowMode {
        app.world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(app.world())
            .expect("janela principal")
            .mode
    }

    /// Testa que "Sim" vira tela cheia sem borda no monitor atual e "Não" vira janela.
    #[test]
    fn fullscreen_value_maps_to_borderless_or_windowed() {
        assert_eq!(
            window_mode_for(true),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        assert_eq!(window_mode_for(false), WindowMode::Windowed);
    }

    /// Testa que mudar o seletor muda o modo da janela principal, nos dois sentidos.
    #[test]
    fn selector_value_is_applied_to_the_primary_window() {
        let mut app = fullscreen_app();
        app.update();
        assert_eq!(primary_window_mode(&mut app), WindowMode::Windowed);

        app.world_mut()
            .resource_mut::<SettingsState>()
            .fullscreen_enabled = true;
        app.update();
        assert_eq!(
            primary_window_mode(&mut app),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );

        app.world_mut()
            .resource_mut::<SettingsState>()
            .fullscreen_enabled = false;
        app.update();
        assert_eq!(primary_window_mode(&mut app), WindowMode::Windowed);
    }
}
