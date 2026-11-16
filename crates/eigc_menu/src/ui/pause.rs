//! Construção do menu de pausa: véu sobre a cena e painel central com os botões.

use super::components::ButtonKind;
use super::spawn::{button_node, fullscreen_node, text_bundle};
use crate::pause::{on_quit_clicked, on_resume_clicked, on_return_to_menu_clicked};
use crate::theme::{self, MenuFonts};
use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::{
    AlignItems, AssetServer, BackgroundColor, BorderColor, BorderRadius, Button,
    ChildSpawnerCommands, Click, Commands, FlexDirection, GlobalZIndex, JustifyContent, Name, Node,
    Pointer, Res, UiRect, Val, default,
};
use bevy::state::state_scoped::DespawnOnExit;
use eigc_moons::PauseState;

/// Largura do painel de pausa, em pixels lógicos.
const PAUSE_PANEL_WIDTH: f32 = 380.0;

/// Monta o menu de pausa ao entrar em `PauseState::Paused`. Some ao sair da pausa, seja por
/// continuar ou por voltar ao menu de seleção (a pausa deixa de existir junto com `Running`).
pub(crate) fn spawn_pause_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Carregar de novo só devolve os handles já em cache; evita depender de `MenuFonts` ter sido
    // inserido pelo menu de seleção antes.
    let fonts = MenuFonts::load(&asset_server);

    commands
        .spawn((
            Name::new("PauseMenu"),
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..fullscreen_node()
            },
            BackgroundColor(theme::PAUSE_VEIL),
            GlobalZIndex(100),
            DespawnOnExit(PauseState::Paused),
        ))
        .with_children(|veil| {
            veil.spawn((
                Name::new("PausePanel"),
                Node {
                    width: Val::Px(PAUSE_PANEL_WIDTH),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(32.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(18.0)),
                    ..default()
                },
                BackgroundColor(theme::PANEL_BACKGROUND),
                BorderColor::all(theme::BORDER_SUBTLE),
            ))
            .with_children(|panel| {
                panel.spawn(text_bundle("PAUSA", &fonts.medium, 14.0, theme::ACCENT));
                panel.spawn(text_bundle(
                    "Exploração pausada",
                    &fonts.bold,
                    32.0,
                    theme::TEXT_PRIMARY,
                ));
                spawn_pause_button(
                    panel,
                    &fonts,
                    "Continuar",
                    ButtonKind::Primary,
                    on_resume_clicked,
                );
                spawn_pause_button(
                    panel,
                    &fonts,
                    "Voltar à seleção de lua",
                    ButtonKind::Ghost,
                    on_return_to_menu_clicked,
                );
                spawn_pause_button(
                    panel,
                    &fonts,
                    "Sair do jogo",
                    ButtonKind::Ghost,
                    on_quit_clicked,
                );
                panel.spawn(text_bundle(
                    "Esc para continuar",
                    &fonts.regular,
                    14.0,
                    theme::TEXT_MUTED,
                ));
            });
        });
}

/// Botão de largura total do painel de pausa, com o observer de clique dado.
fn spawn_pause_button<M>(
    panel: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    label: &str,
    kind: ButtonKind,
    on_click: impl IntoObserverSystem<Pointer<Click>, (), M>,
) {
    let (background, border, text_color, font) = match kind {
        ButtonKind::Primary => (theme::ACCENT, theme::ACCENT, theme::ON_ACCENT, &fonts.bold),
        ButtonKind::Ghost => (
            theme::GHOST_BACKGROUND,
            theme::BORDER_SUBTLE,
            theme::TEXT_PRIMARY,
            &fonts.medium,
        ),
    };

    panel
        .spawn((
            Name::new(format!("PauseButton {label}")),
            Button,
            kind,
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..button_node(Val::Percent(100.0), 0.0)
            },
            BackgroundColor(background),
            BorderColor::all(border),
        ))
        .observe(on_click)
        .with_children(|button| {
            button.spawn(text_bundle(label, font, 17.0, text_color));
        });
}
