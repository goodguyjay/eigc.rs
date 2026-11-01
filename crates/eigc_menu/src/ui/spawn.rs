//! Construção dos nós de UI do menu: título, rótulos das luas, painel de detalhes, aviso e tela
//! de carregamento.

use super::components::{
    ButtonKind, ConfigureControlsRow, DetailPanel, FullscreenOption, MoonLabel, MoonLabelName,
    OverviewUi, PanelField, SettingsGearButton, SettingsGearIcon, SettingsPanel, ToastRoot,
    ToastText,
};
use super::update::panel_right_target;
use crate::interaction::{MenuSelection, on_back_clicked, on_explore_clicked};
use crate::moon_stats::{STAT_LABELS, stat_values};
use crate::settings::{
    on_close_settings_panel_clicked, on_fullscreen_option_clicked, on_settings_button_clicked,
};
use crate::theme::{self, MenuFonts};
use bevy::asset::Handle;
use bevy::prelude::{
    AlignItems, AssetServer, BackgroundColor, BorderColor, BorderRadius, BoxShadow, Bundle, Button,
    ChildSpawnerCommands, Color, Commands, FlexDirection, GlobalZIndex, ImageNode, JustifyContent,
    Name, Node, Overflow, Pickable, PositionType, Res, Text, TextColor, UiRect, UiTransform, Val,
    Visibility, default,
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
            spawn_settings_button(root, &fonts, &asset_server);
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
            spawn_divider(panel, theme::BORDER_SUBTLE);
            panel.spawn((
                PanelField::Summary,
                text_bundle(
                    placeholder.summary,
                    &fonts.regular,
                    16.0,
                    theme::TEXT_SECONDARY,
                ),
            ));
            spawn_divider(panel, theme::BORDER_SUBTLE);
            for (index, label) in STAT_LABELS.iter().enumerate() {
                spawn_stat_row(panel, fonts, index, label, &placeholder_stats[index]);
            }
            spawn_buttons(panel, fonts);
        });
    });
}

/// Linha horizontal fina que separa seções de um painel.
fn spawn_divider(panel: &mut ChildSpawnerCommands, color: Color) {
    panel.spawn((
        Node {
            height: Val::Px(1.0),
            width: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
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

/// Botão de engrenagem (canto superior direito) e o painel que ele abre e fecha. Leva
/// `OverviewUi`, então some junto com o título e os rótulos quando uma lua entra em foco, o que
/// evita o painel de configurações aparecer por cima do painel de detalhes.
fn spawn_settings_button(
    root: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    asset_server: &AssetServer,
) {
    root.spawn((
        Name::new("SettingsButton"),
        OverviewUi,
        SettingsGearButton,
        Button,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(theme::SETTINGS_BUTTON_MARGIN),
            right: Val::Px(theme::SETTINGS_BUTTON_MARGIN),
            width: Val::Px(theme::SETTINGS_BUTTON_SIZE),
            height: Val::Px(theme::SETTINGS_BUTTON_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(theme::SETTINGS_GEAR_BACKGROUND),
        BorderColor::all(theme::SETTINGS_GEAR_BORDER),
    ))
    .observe(on_settings_button_clicked)
    .with_children(|button| {
        button.spawn((
            SettingsGearIcon,
            ImageNode {
                image: asset_server.load("icons/settings-gear.png"),
                color: theme::SETTINGS_TEXT_PRIMARY,
                ..default()
            },
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                ..default()
            },
            UiTransform::default(),
            Pickable::IGNORE,
        ));
    });

    spawn_settings_panel(root, fonts, asset_server);
}

/// Painel de configurações: cabeçalho com título e X, os cards de "Tela cheia" (seletor "Não"/"Sim",
/// só visual) e "Configurar controles" (ainda sem ação), e o rodapé. Nasce oculto; aparece e some
/// conforme `SettingsState`. Leva `OverviewUi` pelo mesmo motivo do botão de engrenagem.
fn spawn_settings_panel(
    root: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    asset_server: &AssetServer,
) {
    root.spawn((
        Name::new("SettingsPanel"),
        SettingsPanel,
        OverviewUi,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(theme::SETTINGS_BUTTON_MARGIN + theme::SETTINGS_BUTTON_SIZE + 12.0),
            right: Val::Px(theme::SETTINGS_BUTTON_MARGIN),
            width: Val::Px(theme::SETTINGS_PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(theme::SETTINGS_PANEL_RADIUS)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(theme::SETTINGS_PANEL_BACKGROUND),
        BorderColor::all(theme::SETTINGS_PANEL_BORDER),
    ))
    .with_children(|panel| {
        spawn_settings_header(panel, fonts, asset_server);
        spawn_settings_cards(panel, fonts);
    });
}

/// Largura em pixels da ilustração `jupiter_orbita.png` (fonte: `assets/icons/README.txt`).
const JUPITER_ORBIT_IMAGE_WIDTH: f32 = 372.0;
/// Altura em pixels da ilustração `jupiter_orbita.png`.
const JUPITER_ORBIT_IMAGE_HEIGHT: f32 = 160.0;
/// Largura exibida da ilustração: 64% da largura do painel, como pedido.
const JUPITER_ORBIT_DISPLAY_WIDTH: f32 = theme::SETTINGS_PANEL_WIDTH * 0.64;
/// Altura exibida, mantendo a proporção original da imagem. Também a altura do cabeçalho inteiro,
/// para não sobrar espaço vazio abaixo da ilustração.
const JUPITER_ORBIT_DISPLAY_HEIGHT: f32 =
    JUPITER_ORBIT_DISPLAY_WIDTH * JUPITER_ORBIT_IMAGE_HEIGHT / JUPITER_ORBIT_IMAGE_WIDTH;

/// Cabeçalho do painel de configurações: a ilustração de Júpiter colada no canto superior direito,
/// sem participar do layout flexível (por isso `position_type: Absolute`, em vez de encolher ou
/// esticar para caber), título e subtítulo por cima dela no canto superior esquerdo, e o X por cima
/// de tudo no canto superior direito. A altura do cabeçalho é a própria altura da ilustração.
fn spawn_settings_header(
    panel: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    asset_server: &AssetServer,
) {
    panel
        .spawn((
            Node {
                height: Val::Px(JUPITER_ORBIT_DISPLAY_HEIGHT),
                padding: UiRect::axes(Val::Px(theme::SETTINGS_ROW_PADDING_X), Val::Px(10.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|header| {
            header.spawn((
                ImageNode {
                    image: asset_server.load("icons/jupiter_orbita.png"),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    right: Val::Px(0.0),
                    width: Val::Px(JUPITER_ORBIT_DISPLAY_WIDTH),
                    height: Val::Px(JUPITER_ORBIT_DISPLAY_HEIGHT),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            header
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|column| {
                    column.spawn(text_bundle(
                        "Configurações",
                        &fonts.bold,
                        18.0,
                        theme::SETTINGS_TEXT_PRIMARY,
                    ));
                    column.spawn(text_bundle(
                        "Ajustes do app",
                        &fonts.regular,
                        12.0,
                        theme::SETTINGS_TEXT_SECONDARY,
                    ));
                });
            header
                .spawn((
                    Name::new("CloseSettingsButton"),
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        right: Val::Px(0.0),
                        width: Val::Px(32.0),
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .observe(on_close_settings_panel_clicked)
                .with_children(|close| {
                    close.spawn(text_bundle(
                        "×",
                        &fonts.medium,
                        22.0,
                        theme::SETTINGS_TEXT_SECONDARY,
                    ));
                });
        });
}

/// Espaçamento interno padrão de uma linha do painel de configurações.
fn settings_row_padding() -> UiRect {
    UiRect::axes(
        Val::Px(theme::SETTINGS_ROW_PADDING_X),
        Val::Px(theme::SETTINGS_ROW_PADDING_Y),
    )
}

/// Título (14px) e descrição (12px, apagada) de duas linhas, usado nas linhas "Tela cheia" e
/// "Configurar controles".
fn spawn_row_text(
    row: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    title: &str,
    description: &str,
) {
    row.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|column| {
        column.spawn(text_bundle(
            title,
            &fonts.medium,
            14.0,
            theme::SETTINGS_TEXT_PRIMARY,
        ));
        column.spawn(text_bundle(
            description,
            &fonts.regular,
            12.0,
            theme::SETTINGS_TEXT_SECONDARY,
        ));
    });
}

/// Contêiner dos dois cards de opção ("Tela cheia" e "Configurar controles"), com espaço entre eles,
/// uma margem lateral para não colarem nas bordas do painel, e a mesma margem embaixo para o painel
/// terminar logo depois do último card.
fn spawn_settings_cards(panel: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    panel
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                margin: UiRect::px(14.0, 14.0, 4.0, 14.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|cards| {
            spawn_fullscreen_row(cards, fonts);
            spawn_configure_controls_row(cards, fonts);
        });
}

/// Card de fundo próprio para uma linha do painel de configurações: borda fina e cantos
/// arredondados, iguais para os dois cards.
fn settings_card_node() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: Val::Px(12.0),
        padding: settings_row_padding(),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(theme::SETTINGS_PANEL_RADIUS)),
        ..default()
    }
}

/// Card "Tela cheia": título e descrição à esquerda, seletor "Não"/"Sim" à direita.
fn spawn_fullscreen_row(cards: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    cards
        .spawn((
            settings_card_node(),
            BackgroundColor(theme::SETTINGS_CARD_BACKGROUND),
            BorderColor::all(theme::SETTINGS_SELECTOR_BORDER),
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            spawn_row_text(row, fonts, "Tela cheia", "Ocupar a tela inteira");
            spawn_fullscreen_selector(row, fonts);
        });
}

/// Seletor "Não"/"Sim" da linha "Tela cheia". Começa em "Não".
fn spawn_fullscreen_selector(row: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    row.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::all(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(theme::SETTINGS_SELECTOR_RADIUS)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(theme::SETTINGS_SELECTOR_BORDER),
    ))
    .with_children(|selector| {
        spawn_fullscreen_option(selector, fonts, FullscreenOption::No, "Não");
        spawn_fullscreen_option(selector, fonts, FullscreenOption::Yes, "Sim");
    });
}

/// Uma opção do seletor "Não"/"Sim". A cor inicial corresponde a `fullscreen_enabled == false`
/// (começa em "Não"); `sync_fullscreen_selector` mantém isso atualizado depois.
fn spawn_fullscreen_option(
    selector: &mut ChildSpawnerCommands,
    fonts: &MenuFonts,
    option: FullscreenOption,
    label: &str,
) {
    let selected = option == FullscreenOption::No;
    let (background, text_color) = if selected {
        (theme::SETTINGS_ACCENT, theme::SETTINGS_PANEL_BACKGROUND)
    } else {
        (Color::NONE, theme::SETTINGS_TEXT_SECONDARY)
    };

    selector
        .spawn((
            option,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(theme::SETTINGS_SELECTOR_OPTION_RADIUS)),
                ..default()
            },
            BackgroundColor(background),
        ))
        .observe(on_fullscreen_option_clicked(option))
        .with_children(|button| {
            button.spawn((option, text_bundle(label, &fonts.medium, 13.0, text_color)));
        });
}

/// Card "Configurar controles": título e descrição à esquerda, chevron à direita. O card inteiro é
/// clicável (sem ação por enquanto) e destaca o fundo em hover.
fn spawn_configure_controls_row(cards: &mut ChildSpawnerCommands, fonts: &MenuFonts) {
    cards
        .spawn((
            Name::new("ConfigureControlsRow"),
            ConfigureControlsRow,
            Button,
            settings_card_node(),
            BackgroundColor(theme::SETTINGS_CARD_BACKGROUND),
            BorderColor::all(theme::SETTINGS_SELECTOR_BORDER),
        ))
        .with_children(|row| {
            spawn_row_text(row, fonts, "Configurar controles", "Teclado e mouse");
            row.spawn(text_bundle(
                ">",
                &fonts.medium,
                16.0,
                theme::SETTINGS_TEXT_SECONDARY,
            ));
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
