//! Construção dos nós de UI do menu: título, rótulos das luas, painel de detalhes, aviso e tela
//! de carregamento.

use super::components::{
    ButtonKind, DetailPanel, MoonLabel, MoonLabelName, OverviewUi, PanelField, ToastRoot, ToastText,
};
use super::update::panel_right_target;
use crate::interaction::{MenuSelection, on_back_clicked, on_explore_clicked};
use crate::moon_stats::{STAT_LABELS, stat_values};
use crate::theme::{self, MenuFonts};
use bevy::asset::Handle;
use bevy::prelude::{
    AlignItems, AssetServer, BackgroundColor, BorderColor, BorderRadius, BoxShadow, Bundle, Button,
    ChildSpawnerCommands, Color, Commands, FlexDirection, GlobalZIndex, JustifyContent, Name, Node,
    Pickable, PositionType, Res, Text, TextColor, UiRect, Val, Visibility, default,
};
use bevy::state::state_scoped::DespawnOnEnter;
use bevy::text::{Font, Justify, TextFont, TextLayout};
use eigc_moons::{AppState, MOON_DISPLAY_ORDER, MoonId, moon_info};

/// Texto não interativo: nunca bloqueia o picking dos elementos abaixo dele.
fn text_bundle(
    text: impl Into<String>,
    font: &Handle<Font>,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    )
}

/// Nó que cobre a janela inteira, posicionado de forma absoluta.
fn fullscreen_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

/// Monta toda a UI do menu ao entrar em `MainMenu`. Os nós são removidos ao entrar em `Running`.
pub(crate) fn spawn_menu_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let fonts = MenuFonts::load(&asset_server);
    commands.insert_resource(fonts.clone());

    commands
        .spawn((
            Name::new("MenuUiRoot"),
            fullscreen_node(),
            Pickable::IGNORE,
            DespawnOnEnter(AppState::Running),
        ))
        .with_children(|root| {
            spawn_title(root, &fonts);
            for moon_id in MOON_DISPLAY_ORDER {
                spawn_moon_label(root, &fonts, moon_id);
            }
            spawn_detail_panel(root, &fonts);
            spawn_toast(root, &fonts);
        });
}

/// Título e subtítulo da visão geral, no topo e centralizados.
fn spawn_title(root: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    root.spawn((
        Name::new("MenuTitle"),
        OverviewUi,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(52.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|title| {
        title.spawn(text_bundle(
            "SISTEMA JOVIANO",
            &fonts.medium,
            14.0,
            theme::ACCENT,
        ));
        title.spawn(text_bundle(
            "Luas Galileanas",
            &fonts.bold,
            58.0,
            theme::TEXT_PRIMARY,
        ));
        title.spawn(text_bundle(
            "Escolha uma lua para iniciar a exploração terrestre",
            &fonts.regular,
            18.0,
            theme::TEXT_SECONDARY,
        ));
    });
}

/// Rótulo de uma lua na visão geral: nome e selo de disponibilidade. A posição na tela é
/// atualizada a cada quadro, a partir da posição 3D da lua.
fn spawn_moon_label(root: &mut ChildSpawnerCommands, fonts: &MenuFonts, moon_id: MoonId) {
    let info = moon_info(moon_id);
    let (badge_text, badge_background, badge_color) = if info.available {
        (
            "Disponível",
            theme::BADGE_AVAILABLE_BACKGROUND,
            theme::ACCENT,
        )
    } else {
        ("Em breve", theme::BADGE_SOON_BACKGROUND, theme::TEXT_MUTED)
    };

    root.spawn((
        Name::new(format!("MoonLabel {moon_id:?}")),
        MoonLabel(moon_id),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-1_000.0),
            top: Val::Px(-1_000.0),
            width: Val::Px(theme::LABEL_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|label| {
        label.spawn((
            MoonLabelName(moon_id),
            text_bundle(info.display_name, &fonts.semibold, 26.0, theme::LABEL_IDLE),
        ));
        label
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(badge_background),
                Pickable::IGNORE,
            ))
            .with_children(|badge| {
                badge.spawn(text_bundle(badge_text, &fonts.medium, 12.0, badge_color));
            });
    });
}

/// Painel de detalhes da lua em foco, ancorado à direita. Nasce fora da tela e oculto; o conteúdo
/// é preenchido quando uma lua recebe foco.
fn spawn_detail_panel(root: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    let placeholder = moon_info(MOON_DISPLAY_ORDER[0]);
    let placeholder_stats = stat_values(placeholder);

    root.spawn((
        Name::new("DetailPanelDock"),
        DetailPanel,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            right: Val::Px(panel_right_target(false)),
            width: Val::Px(theme::PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|dock| {
        dock.spawn((
            Name::new("DetailPanel"),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(32.0)),
                row_gap: Val::Px(18.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(theme::PANEL_BACKGROUND),
            BorderColor::all(theme::BORDER_SUBTLE),
            BoxShadow::new(
                theme::PANEL_SHADOW,
                Val::Px(0.0),
                Val::Px(24.0),
                Val::Px(0.0),
                Val::Px(60.0),
            ),
        ))
        .with_children(|panel| {
            panel.spawn(text_bundle(
                "LUA GALILEANA",
                &fonts.medium,
                13.0,
                theme::ACCENT,
            ));
            panel.spawn((
                PanelField::Name,
                text_bundle(
                    placeholder.display_name,
                    &fonts.bold,
                    46.0,
                    theme::TEXT_PRIMARY,
                ),
            ));
            panel.spawn((
                PanelField::Tagline,
                text_bundle(placeholder.tagline, &fonts.medium, 18.0, theme::ACCENT),
            ));
            spawn_divider(panel);
            panel.spawn((
                PanelField::Summary,
                text_bundle(
                    placeholder.summary,
                    &fonts.regular,
                    16.0,
                    theme::TEXT_SECONDARY,
                ),
            ));
            spawn_divider(panel);
            for (index, label) in STAT_LABELS.iter().enumerate() {
                spawn_stat_row(panel, fonts, index, label, &placeholder_stats[index]);
            }
            spawn_buttons(panel, fonts);
        });
    });
}

/// Linha horizontal fina que separa seções do painel.
fn spawn_divider(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Node {
            height: Val::Px(1.0),
            width: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme::BORDER_SUBTLE),
        Pickable::IGNORE,
    ));
}

/// Linha de estatística: rótulo à esquerda e valor à direita.
fn spawn_stat_row(
    panel: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    index: usize,
    label: &str,
    value: &str,
) {
    panel
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(16.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            row.spawn(text_bundle(label, &fonts.regular, 15.0, theme::TEXT_MUTED));
            row.spawn((
                PanelField::Stat(index),
                TextLayout::new_with_justify(Justify::Right),
                text_bundle(value, &fonts.medium, 15.0, theme::TEXT_PRIMARY),
            ));
        });
}

/// Botões "Explorar" (principal) e "Voltar" (secundário) do painel.
fn spawn_buttons(panel: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(12.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Name::new("ExploreButton"),
                Button,
                ButtonKind::Primary,
                button_node(Val::Auto, 1.0),
                BackgroundColor(theme::ACCENT),
                BorderColor::all(theme::ACCENT),
            ))
            .observe(on_explore_clicked)
            .with_children(|button| {
                button.spawn(text_bundle("Explorar", &fonts.bold, 17.0, theme::ON_ACCENT));
            });

            row.spawn((
                Name::new("BackButton"),
                Button,
                ButtonKind::Ghost,
                button_node(Val::Px(120.0), 0.0),
                BackgroundColor(theme::GHOST_BACKGROUND),
                BorderColor::all(theme::BORDER_SUBTLE),
            ))
            .observe(on_back_clicked)
            .with_children(|button| {
                button.spawn(text_bundle(
                    "Voltar",
                    &fonts.medium,
                    16.0,
                    theme::TEXT_PRIMARY,
                ));
            });
        });
}

/// Geometria compartilhada pelos botões do painel.
fn button_node(width: Val, flex_grow: f32) -> Node {
    Node {
        width,
        height: Val::Px(52.0),
        flex_grow,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(14.0)),
        ..default()
    }
}

/// Aviso de "não implementada", no rodapé. Nasce oculto e só é exibido quando necessário.
fn spawn_toast(root: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    root.spawn((
        Name::new("Toast"),
        ToastRoot,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(56.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|toast| {
        toast
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(14.0)),
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(theme::TOAST_BACKGROUND),
                BorderColor::all(theme::TOAST_ACCENT),
                Pickable::IGNORE,
            ))
            .with_children(|pill| {
                pill.spawn((
                    Node {
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(theme::TOAST_ACCENT),
                    Pickable::IGNORE,
                ));
                pill.spawn((
                    ToastText,
                    text_bundle("", &fonts.medium, 16.0, theme::TEXT_PRIMARY),
                ));
            });
    });
}

/// Tela de carregamento exibida entre o clique em "Explorar" e o início da simulação.
pub(crate) fn spawn_loading_overlay(
    mut commands: Commands,
    fonts: Res<MenuFonts>,
    selection: Res<MenuSelection>,
) {
    let moon_name = selection
        .focused
        .map(|moon_id| moon_info(moon_id).display_name)
        .unwrap_or("");

    commands
        .spawn((
            Name::new("LoadingOverlay"),
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..fullscreen_node()
            },
            BackgroundColor(theme::LOADING_VEIL),
            GlobalZIndex(100),
            DespawnOnEnter(AppState::Running),
        ))
        .with_children(|overlay| {
            overlay.spawn(text_bundle(
                "PREPARANDO EXPLORAÇÃO",
                &fonts.medium,
                14.0,
                theme::ACCENT,
            ));
            overlay.spawn(text_bundle(
                moon_name,
                &fonts.bold,
                64.0,
                theme::TEXT_PRIMARY,
            ));
            overlay.spawn(text_bundle(
                "Carregando terreno e céu...",
                &fonts.regular,
                18.0,
                theme::TEXT_SECONDARY,
            ));
        });
}
