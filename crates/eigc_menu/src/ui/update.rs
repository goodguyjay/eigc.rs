//! Sistemas que mantêm a UI do menu em sincronia com a seleção: conteúdo e animação do painel,
//! rótulos ancorados às luas, estilo dos botões e aviso de "não implementada".

use super::components::{
    ButtonKind, DetailPanel, MoonLabel, MoonLabelName, OverviewUi, PanelField, ToastRoot,
    ToastState, ToastText,
};
use crate::explore::NotImplementedNotice;
use crate::interaction::MenuSelection;
use crate::layout::{damp_f32, label_anchor_world};
use crate::moon_stats::stat_values;
use crate::scene::MenuCamera;
use crate::theme;
use bevy::prelude::{
    BackgroundColor, BorderColor, Camera, Changed, Color, DetectChanges, GlobalTransform,
    Interaction, MessageReader, Node, Or, Query, Res, ResMut, Single, Text, TextColor, Time, Val,
    Visibility, With,
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

/// Atualiza as cores dos botões quando a interação com eles muda.
pub(crate) fn style_buttons(
    mut buttons: Query<
        (
            &Interaction,
            &ButtonKind,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, kind, mut background, mut border) in &mut buttons {
        let (background_color, border_color) = button_colors(*kind, *interaction);
        background.0 = background_color;
        *border = BorderColor::all(border_color);
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

    #[test]
    fn open_panel_sits_on_the_margin_and_closed_panel_is_off_screen() {
        assert_eq!(panel_right_target(true), theme::PANEL_MARGIN);
        assert!(panel_right_target(false) < -theme::PANEL_WIDTH);
    }

    #[test]
    fn panel_is_hidden_only_after_leaving_the_screen_without_focus() {
        let closed = panel_right_target(false);
        assert!(panel_is_hidden(closed, false));
        assert!(!panel_is_hidden(closed, true));
        assert!(!panel_is_hidden(theme::PANEL_MARGIN, false));
    }

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

    #[test]
    fn buttons_change_colors_on_hover_and_press() {
        for kind in [ButtonKind::Primary, ButtonKind::Ghost] {
            let idle = button_colors(kind, Interaction::None);
            assert_ne!(idle, button_colors(kind, Interaction::Hovered));
            assert_ne!(idle, button_colors(kind, Interaction::Pressed));
        }
    }

    #[test]
    fn toast_message_names_the_moon() {
        assert_eq!(
            toast_message(MoonId::Ganymede),
            "Exploração de Ganimedes ainda não implementada"
        );
    }
}
