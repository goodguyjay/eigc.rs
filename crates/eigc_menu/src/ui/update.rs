//! Sistemas que mantêm a UI do menu em sincronia com a seleção: conteúdo e animação do painel,
//! rótulos ancorados às luas, estilo dos botões e aviso de "não implementada".

use super::components::{
    ButtonKind, ConfigureControlsRow, DetailPanel, FullscreenOption, MoonLabel, MoonLabelName,
    OverviewUi, PanelField, SettingsGearButton, SettingsGearIcon, SettingsPanel, ToastRoot,
    ToastState, ToastText,
};
use crate::explore::NotImplementedNotice;
use crate::interaction::MenuSelection;
use crate::layout::{damp_f32, label_anchor_world};
use crate::moon_stats::stat_values;
use crate::scene::MenuCamera;
use crate::settings::SettingsState;
use crate::theme;
use bevy::prelude::{
    BackgroundColor, BorderColor, Button, Camera, Changed, Color, DetectChanges, GlobalTransform,
    Interaction, MessageReader, Node, Or, Query, Res, ResMut, Rot2, Single, Text, TextColor, Time,
    UiTransform, Val, Visibility, With,
};
use eigc_moons::{MoonId, MoonInfo, moon_info};

/// Taxa de amortecimento da entrada e saída do painel de detalhes, em 1/s.
const PANEL_SLIDE_RATE: f32 = 7.0;

/// Folga extra, além da largura do painel, para ele ficar totalmente fora da tela quando oculto.
const PANEL_HIDDEN_EXTRA: f32 = 120.0;

/// Quanto tempo o aviso de "não implementada" fica visível, em segundos.
const TOAST_DURATION_SECS: f32 = 3.0;

/// Deslocamento alvo do painel em relação à borda direita: aberto na margem, ou fora da tela.
pub(crate) fn panel_right_target(focused: bool) -> f32 {
    if focused {
        theme::PANEL_MARGIN
    } else {
        -(theme::PANEL_WIDTH + PANEL_HIDDEN_EXTRA)
    }
}

/// Indica se o painel já terminou de sair da tela e pode ficar oculto.
fn panel_is_hidden(current_right: f32, focused: bool) -> bool {
    !focused && current_right <= panel_right_target(false) + 1.0
}

/// Texto de um campo do painel de detalhes para a lua dada.
fn panel_field_text(field: PanelField, info: &MoonInfo, stats: &[String; 5]) -> String {
    match field {
        PanelField::Name => info.display_name.to_string(),
        PanelField::Tagline => info.tagline.to_string(),
        PanelField::Summary => info.summary.to_string(),
        PanelField::Stat(index) => stats.get(index).cloned().unwrap_or_default(),
    }
}

/// Cores de fundo e borda de um botão, conforme o tipo e a interação atual.
fn button_colors(kind: ButtonKind, interaction: Interaction) -> (Color, Color) {
    match (kind, interaction) {
        (ButtonKind::Primary, Interaction::None) => (theme::ACCENT, theme::ACCENT),
        (ButtonKind::Primary, Interaction::Hovered) => (theme::ACCENT_HOVER, theme::ACCENT_HOVER),
        (ButtonKind::Primary, Interaction::Pressed) => {
            (theme::ACCENT_PRESSED, theme::ACCENT_PRESSED)
        }
        (ButtonKind::Ghost, Interaction::None) => (theme::GHOST_BACKGROUND, theme::BORDER_SUBTLE),
        (ButtonKind::Ghost, Interaction::Hovered) => {
            (theme::GHOST_HOVER_BACKGROUND, theme::BORDER_STRONG)
        }
        (ButtonKind::Ghost, Interaction::Pressed) => {
            (theme::GHOST_HOVER_BACKGROUND, theme::TEXT_PRIMARY)
        }
    }
}

/// Mensagem do aviso exibido ao tentar usar uma lua ainda não implementada.
fn toast_message(moon_id: MoonId) -> String {
    format!(
        "Exploração de {} ainda não implementada",
        moon_info(moon_id).display_name
    )
}

/// Preenche o painel de detalhes com os dados da lua em foco. Ao perder o foco, mantém o conteúdo
/// anterior para que o painel não mude de texto enquanto desliza para fora.
pub(crate) fn refresh_panel_content(
    selection: Res<MenuSelection>,
    mut fields: Query<(&PanelField, &mut Text)>,
) {
    if !selection.is_changed() {
        return;
    }
    let Some(moon_id) = selection.focused else {
        return;
    };
    let info = moon_info(moon_id);
    let stats = stat_values(info);
    for (field, mut text) in &mut fields {
        text.0 = panel_field_text(*field, info, &stats);
    }
}

/// Desliza o painel de detalhes para dentro da tela quando há lua em foco, e para fora quando não.
pub(crate) fn slide_detail_panel(
    time: Res<Time>,
    selection: Res<MenuSelection>,
    panel: Single<(&mut Node, &mut Visibility), With<DetailPanel>>,
) {
    let (mut node, mut visibility) = panel.into_inner();
    let focused = selection.focused.is_some();
    let target = panel_right_target(focused);
    let current = match node.right {
        Val::Px(value) => value,
        _ => target,
    };
    let next = damp_f32(current, target, PANEL_SLIDE_RATE, time.delta_secs());
    node.right = Val::Px(next);
    *visibility = if panel_is_hidden(next, focused) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
}

/// Esconde título e rótulos da visão geral enquanto uma lua está em foco.
pub(crate) fn update_overview_visibility(
    selection: Res<MenuSelection>,
    mut overview: Query<&mut Visibility, Or<(With<OverviewUi>, With<MoonLabel>)>>,
) {
    if !selection.is_changed() {
        return;
    }
    let wanted = if selection.focused.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut overview {
        *visibility = wanted;
    }
}

/// Posiciona cada rótulo sob a sua lua, convertendo a posição 3D para coordenadas de tela.
pub(crate) fn position_moon_labels(
    camera: Single<(&Camera, &GlobalTransform), With<MenuCamera>>,
    mut labels: Query<(&MoonLabel, &mut Node)>,
) {
    let (camera, camera_transform) = camera.into_inner();
    for (label, mut node) in &mut labels {
        let Ok(viewport) = camera.world_to_viewport(camera_transform, label_anchor_world(label.0))
        else {
            continue;
        };
        node.left = Val::Px(viewport.x - theme::LABEL_WIDTH / 2.0);
        node.top = Val::Px(viewport.y);
    }
}

/// Destaca o nome da lua que está sob o cursor ou em foco.
pub(crate) fn highlight_labels(
    selection: Res<MenuSelection>,
    mut names: Query<(&MoonLabelName, &mut TextColor)>,
) {
    if !selection.is_changed() {
        return;
    }
    for (name, mut color) in &mut names {
        let is_active = selection.hovered == Some(name.0) || selection.focused == Some(name.0);
        color.0 = if is_active {
            theme::TEXT_PRIMARY
        } else {
            theme::LABEL_IDLE
        };
    }
}

/// Atualiza as cores dos botões quando a interação com eles muda, ou quando o próprio
/// `ButtonKind` muda (ex.: o alternador de tela cheia, que troca de tipo ao ser ligado/desligado).
#[allow(clippy::type_complexity)]
pub(crate) fn style_buttons(
    mut buttons: Query<
        (
            &Interaction,
            &ButtonKind,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Or<(Changed<Interaction>, Changed<ButtonKind>)>,
    >,
) {
    for (interaction, kind, mut background, mut border) in &mut buttons {
        let (background_color, border_color) = button_colors(*kind, *interaction);
        background.0 = background_color;
        *border = BorderColor::all(border_color);
    }
}

/// Se o painel de configurações deve estar visível: aberto e sem nenhuma lua em foco. O painel de
/// detalhes de uma lua ocupa a mesma região da tela (lado direito), então os dois nunca aparecem
/// ao mesmo tempo.
pub(crate) fn settings_panel_is_visible(panel_open: bool, moon_focused: bool) -> bool {
    panel_open && !moon_focused
}

/// Se a opção do seletor de tela cheia está selecionada, dado o valor atual de
/// `fullscreen_enabled`. "Não" é a opção inicial.
pub(crate) fn fullscreen_option_is_selected(
    option: FullscreenOption,
    fullscreen_enabled: bool,
) -> bool {
    (option == FullscreenOption::Yes) == fullscreen_enabled
}

/// Fundo de uma opção do seletor: destacado se selecionada, transparente caso contrário.
pub(crate) fn fullscreen_option_background(
    option: FullscreenOption,
    fullscreen_enabled: bool,
) -> Color {
    if fullscreen_option_is_selected(option, fullscreen_enabled) {
        theme::SETTINGS_ACCENT
    } else {
        Color::NONE
    }
}

/// Cor do texto de uma opção do seletor: escura sobre o fundo destacado, apagada caso contrário.
pub(crate) fn fullscreen_option_text_color(
    option: FullscreenOption,
    fullscreen_enabled: bool,
) -> Color {
    if fullscreen_option_is_selected(option, fullscreen_enabled) {
        theme::SETTINGS_PANEL_BACKGROUND
    } else {
        theme::SETTINGS_TEXT_SECONDARY
    }
}

/// Fundo da linha "Configurar controles": destacado em hover e pressionado, transparente em
/// repouso.
pub(crate) fn configure_controls_row_background(interaction: Interaction) -> Color {
    match interaction {
        Interaction::Hovered | Interaction::Pressed => theme::SETTINGS_CARD_HOVER_BACKGROUND,
        Interaction::None => theme::SETTINGS_CARD_BACKGROUND,
    }
}

/// Cor da borda da engrenagem: destacada com o painel aberto ou o cursor sobre ela.
pub(crate) fn settings_gear_border(interaction: Interaction, panel_open: bool) -> Color {
    if panel_open || interaction != Interaction::None {
        theme::SETTINGS_GEAR_BORDER_ACTIVE
    } else {
        theme::SETTINGS_GEAR_BORDER
    }
}

/// Ângulo do ícone da engrenagem: gira ao abrir o painel.
pub(crate) fn settings_gear_rotation_degrees(panel_open: bool) -> f32 {
    if panel_open { 60.0 } else { 0.0 }
}

/// Mostra ou esconde o painel de configurações conforme `SettingsState` e a lua em foco.
pub(crate) fn toggle_settings_panel_visibility(
    settings: Res<SettingsState>,
    selection: Res<MenuSelection>,
    mut panel: Query<&mut Visibility, With<SettingsPanel>>,
) {
    if !settings.is_changed() && !selection.is_changed() {
        return;
    }
    let Ok(mut visibility) = panel.single_mut() else {
        return;
    };
    *visibility = if settings_panel_is_visible(settings.panel_open, selection.focused.is_some()) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

/// Atualiza o fundo e a cor do texto de cada opção do seletor de tela cheia conforme
/// `SettingsState`. As opções (botão) e seus textos são entidades separadas, ambas marcadas com o
/// mesmo `FullscreenOption`, então duas queries bastam, sem precisar navegar a hierarquia.
pub(crate) fn sync_fullscreen_selector(
    settings: Res<SettingsState>,
    mut options: Query<(&FullscreenOption, &mut BackgroundColor), With<Button>>,
    mut labels: Query<(&FullscreenOption, &mut TextColor)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (option, mut background) in &mut options {
        background.0 = fullscreen_option_background(*option, settings.fullscreen_enabled);
    }
    for (option, mut color) in &mut labels {
        color.0 = fullscreen_option_text_color(*option, settings.fullscreen_enabled);
    }
}

/// Destaca o fundo da linha "Configurar controles" em hover e ao pressionar.
#[allow(clippy::type_complexity)]
pub(crate) fn style_configure_controls_row(
    mut rows: Query<
        (&Interaction, &mut BackgroundColor),
        (With<ConfigureControlsRow>, Changed<Interaction>),
    >,
) {
    for (interaction, mut background) in &mut rows {
        background.0 = configure_controls_row_background(*interaction);
    }
}

/// Atualiza a borda e a rotação da engrenagem conforme a interação e `SettingsState::panel_open`.
/// Roda a cada quadro (é uma única entidade pequena): fechar o painel pelo X, por exemplo, muda
/// `panel_open` sem mudar a interação da própria engrenagem, então não dá para depender só de
/// `Changed<Interaction>`.
pub(crate) fn style_settings_gear(
    settings: Res<SettingsState>,
    mut gear: Query<(&Interaction, &mut BorderColor), With<SettingsGearButton>>,
    mut icon: Query<&mut UiTransform, With<SettingsGearIcon>>,
) {
    if let Ok((interaction, mut border)) = gear.single_mut() {
        *border = BorderColor::all(settings_gear_border(*interaction, settings.panel_open));
    }
    if let Ok(mut transform) = icon.single_mut() {
        transform.rotation = Rot2::degrees(settings_gear_rotation_degrees(settings.panel_open));
    }
}

/// Exibe o aviso de "não implementada" ao receber um pedido e o esconde depois de alguns segundos.
pub(crate) fn update_toast(
    time: Res<Time>,
    mut notices: MessageReader<NotImplementedNotice>,
    mut state: ResMut<ToastState>,
    mut text: Single<&mut Text, With<ToastText>>,
    mut root: Single<&mut Visibility, With<ToastRoot>>,
) {
    if let Some(notice) = notices.read().last() {
        text.0 = toast_message(notice.0);
        state.remaining_secs = TOAST_DURATION_SECS;
    } else {
        state.remaining_secs = (state.remaining_secs - time.delta_secs()).max(0.0);
    }
    **root = if state.remaining_secs > 0.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que o painel aberto fica na margem direita e o fechado fica
    /// totalmente fora da tela.
    #[test]
    fn open_panel_sits_on_the_margin_and_closed_panel_is_off_screen() {
        assert_eq!(panel_right_target(true), theme::PANEL_MARGIN);
        assert!(panel_right_target(false) < -theme::PANEL_WIDTH);
    }

    /// Testa que o painel só é ocultado depois de sair da tela sem foco, e
    /// nunca enquanto há lua em foco ou ele ainda está aberto.
    #[test]
    fn panel_is_hidden_only_after_leaving_the_screen_without_focus() {
        let closed = panel_right_target(false);
        assert!(panel_is_hidden(closed, false));
        assert!(!panel_is_hidden(closed, true));
        assert!(!panel_is_hidden(theme::PANEL_MARGIN, false));
    }

    /// Testa que os campos do painel (nome, resumo e estatística) vêm da lua em
    /// foco, e que um índice de estatística inexistente resulta em texto vazio.
    #[test]
    fn panel_fields_come_from_the_focused_moon() {
        let info = moon_info(MoonId::Europa);
        let stats = stat_values(info);
        assert_eq!(panel_field_text(PanelField::Name, info, &stats), "Europa");
        assert_eq!(
            panel_field_text(PanelField::Summary, info, &stats),
            info.summary
        );
        assert_eq!(
            panel_field_text(PanelField::Stat(0), info, &stats),
            "3.122 km"
        );
        assert_eq!(panel_field_text(PanelField::Stat(99), info, &stats), "");
    }

    /// Testa que os dois tipos de botão mudam de cor ao receber hover e ao
    /// serem pressionados.
    #[test]
    fn buttons_change_colors_on_hover_and_press() {
        for kind in [ButtonKind::Primary, ButtonKind::Ghost] {
            let idle = button_colors(kind, Interaction::None);
            assert_ne!(idle, button_colors(kind, Interaction::Hovered));
            assert_ne!(idle, button_colors(kind, Interaction::Pressed));
        }
    }

    /// Testa que a mensagem do aviso cita o nome da lua em português
    /// (`Ganimedes`).
    #[test]
    fn toast_message_names_the_moon() {
        assert_eq!(
            toast_message(MoonId::Ganymede),
            "Exploração de Ganimedes ainda não implementada"
        );
    }

    /// Testa que o painel de configurações abre e fecha pela engrenagem (`panel_open`), e que
    /// nunca fica visível com uma lua em foco, mesmo que esteja aberto.
    #[test]
    fn settings_panel_opens_and_closes_with_the_gear_and_never_over_the_detail_panel() {
        assert!(!settings_panel_is_visible(false, false));
        assert!(settings_panel_is_visible(true, false));
        assert!(!settings_panel_is_visible(true, true));
    }

    /// Testa que o seletor de tela cheia começa em "Não" e troca de opção selecionada conforme
    /// `fullscreen_enabled`.
    #[test]
    fn fullscreen_selector_starts_on_no_and_flips_with_the_value() {
        assert!(fullscreen_option_is_selected(FullscreenOption::No, false));
        assert!(!fullscreen_option_is_selected(FullscreenOption::Yes, false));
        assert!(fullscreen_option_is_selected(FullscreenOption::Yes, true));
        assert!(!fullscreen_option_is_selected(FullscreenOption::No, true));
    }

    /// Testa que só a opção selecionada do seletor fica com o fundo e o texto destacados.
    #[test]
    fn fullscreen_selected_option_uses_the_highlighted_colors() {
        assert_eq!(
            fullscreen_option_background(FullscreenOption::No, false),
            theme::SETTINGS_ACCENT
        );
        assert_eq!(
            fullscreen_option_background(FullscreenOption::Yes, false),
            Color::NONE
        );
        assert_eq!(
            fullscreen_option_text_color(FullscreenOption::No, false),
            theme::SETTINGS_PANEL_BACKGROUND
        );
        assert_eq!(
            fullscreen_option_text_color(FullscreenOption::Yes, false),
            theme::SETTINGS_TEXT_SECONDARY
        );
    }

    /// Testa que a linha "Configurar controles" só destaca o fundo em hover e ao pressionar.
    #[test]
    fn configure_controls_row_highlights_on_hover_and_press() {
        assert_eq!(
            configure_controls_row_background(Interaction::None),
            theme::SETTINGS_CARD_BACKGROUND
        );
        assert_eq!(
            configure_controls_row_background(Interaction::Hovered),
            theme::SETTINGS_CARD_HOVER_BACKGROUND
        );
        assert_eq!(
            configure_controls_row_background(Interaction::Pressed),
            theme::SETTINGS_CARD_HOVER_BACKGROUND
        );
    }

    /// Testa que a borda da engrenagem destaca com hover ou com o painel aberto, mesmo sem hover.
    #[test]
    fn settings_gear_border_activates_on_hover_or_open_panel() {
        assert_eq!(
            settings_gear_border(Interaction::None, false),
            theme::SETTINGS_GEAR_BORDER
        );
        assert_eq!(
            settings_gear_border(Interaction::Hovered, false),
            theme::SETTINGS_GEAR_BORDER_ACTIVE
        );
        assert_eq!(
            settings_gear_border(Interaction::None, true),
            theme::SETTINGS_GEAR_BORDER_ACTIVE
        );
    }

    /// Testa que o ícone da engrenagem gira 60 graus quando o painel abre, e volta a 0 quando
    /// fecha.
    #[test]
    fn settings_gear_icon_rotates_sixty_degrees_when_the_panel_opens() {
        assert_eq!(settings_gear_rotation_degrees(false), 0.0);
        assert_eq!(settings_gear_rotation_degrees(true), 60.0);
    }
}
