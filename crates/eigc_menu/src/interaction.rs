//! Interação do menu com o mouse e o teclado: hover, clique, foco numa lua e animações de câmera
//! e escala que decorrem disso.

use crate::explore::{ExploreRequested, NotImplementedNotice};
use crate::layout::{active_scale_factor, camera_target, damp_towards, moon_uniform_scale};
use crate::scene::{MenuCamera, MenuMoon};
use bevy::picking::mesh_picking::MeshPickingSettings;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::{
    ButtonInput, Click, EntityCommands, KeyCode, MessageWriter, On, Out, Over, Pointer, Query, Res,
    ResMut, Resource, Single, State, Time, Transform, Vec3, With,
};
use eigc_moons::{AppState, MoonId, MoonInfo, moon_info};

/// Taxa de amortecimento do movimento da câmera entre visão geral e foco, em 1/s.
const CAMERA_DAMPING_RATE: f32 = 4.0;

/// Taxa de amortecimento da escala das luas ao entrar ou sair de hover, em 1/s.
const SCALE_DAMPING_RATE: f32 = 10.0;

/// Única fonte de verdade sobre qual lua está sob o cursor e qual está em foco no menu.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuSelection {
    /// Lua atualmente sob o cursor, se houver.
    pub hovered: Option<MoonId>,
    /// Lua em foco (câmera aproximada e ficha aberta), se houver.
    pub focused: Option<MoonId>,
}

/// Efeito de um clique numa lua do menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClickOutcome {
    /// Dar foco na lua: aproximar a câmera e abrir a ficha.
    Focus(MoonId),
    /// A lua ainda não está implementada: avisar o jogador, sem dar foco.
    NotImplemented(MoonId),
    /// O clique não muda nada, por exemplo clicar na lua que já está em foco.
    Ignore,
}

/// Decide o efeito de clicar numa lua, dado o estado atual da seleção.
pub(crate) fn resolve_click(info: &MoonInfo, selection: &MenuSelection) -> ClickOutcome {
    if !info.available {
        ClickOutcome::NotImplemented(info.moon_id)
    } else if selection.focused == Some(info.moon_id) {
        ClickOutcome::Ignore
    } else {
        ClickOutcome::Focus(info.moon_id)
    }
}

/// Liga à entidade raiz de uma lua os observers de hover e clique.
///
/// Os eventos de picking nascem nos meshes filhos do `SceneRoot` e sobem até a raiz, onde estes
/// observers estão.
pub(crate) fn attach_moon_observers(entity: &mut EntityCommands, moon_id: MoonId) {
    entity
        .observe(
            move |_: On<Pointer<Over>>, mut selection: ResMut<MenuSelection>| {
                selection.hovered = Some(moon_id);
            },
        )
        .observe(
            move |_: On<Pointer<Out>>, mut selection: ResMut<MenuSelection>| {
                // O Over de outra lua pode chegar antes deste Out; só limpa se ainda for esta.
                if selection.hovered == Some(moon_id) {
                    selection.hovered = None;
                }
            },
        )
        .observe(
            move |click: On<Pointer<Click>>,
                  state: Res<State<AppState>>,
                  mut selection: ResMut<MenuSelection>,
                  mut notices: MessageWriter<NotImplementedNotice>| {
                if click.button != PointerButton::Primary || *state.get() != AppState::MainMenu {
                    return;
                }
                match resolve_click(moon_info(moon_id), &selection) {
                    ClickOutcome::Focus(focused) => selection.focused = Some(focused),
                    ClickOutcome::NotImplemented(unavailable) => {
                        notices.write(NotImplementedNotice(unavailable));
                    }
                    ClickOutcome::Ignore => {}
                }
            },
        );
}

/// Clique em "Explorar": pede a exploração da lua em foco.
pub(crate) fn on_explore_clicked(
    click: On<Pointer<Click>>,
    selection: Res<MenuSelection>,
    mut requests: MessageWriter<ExploreRequested>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Some(moon_id) = selection.focused {
        requests.write(ExploreRequested(moon_id));
    }
}

/// Clique em "Voltar": tira o foco da lua e devolve a câmera à visão geral.
pub(crate) fn on_back_clicked(click: On<Pointer<Click>>, mut selection: ResMut<MenuSelection>) {
    if click.button == PointerButton::Primary {
        selection.focused = None;
    }
}

/// Encerra o picking de malhas ao sair do menu.
///
/// O `MeshPickingPlugin` não pode ser removido depois de adicionado, e sem esta restrição ele
/// lançaria um raio por quadro contra todas as malhas visíveis da simulação (terreno, céu). Com
/// `require_markers` ligado, o backend ignora câmeras sem `MeshPickingCamera`, e nenhuma câmera da
/// simulação tem esse marcador.
pub(crate) fn restrict_mesh_picking_to_marked_cameras(mut settings: ResMut<MeshPickingSettings>) {
    settings.require_markers = true;
}

/// ESC tira o foco da lua e devolve a câmera à visão geral.
pub(crate) fn exit_focus_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<MenuSelection>,
) {
    if keyboard.just_pressed(KeyCode::Escape) && selection.focused.is_some() {
        selection.focused = None;
    }
}

/// Move a câmera do menu, com amortecimento, para a posição da visão geral ou da lua em foco.
pub(crate) fn animate_menu_camera(
    time: Res<Time>,
    selection: Res<MenuSelection>,
    mut camera: Single<&mut Transform, With<MenuCamera>>,
) {
    camera.translation = damp_towards(
        camera.translation,
        camera_target(selection.focused),
        CAMERA_DAMPING_RATE,
        time.delta_secs(),
    );
}

/// Amplia levemente a lua sob o cursor ou em foco, com amortecimento.
pub(crate) fn animate_moon_scale(
    time: Res<Time>,
    selection: Res<MenuSelection>,
    mut moons: Query<(&MenuMoon, &mut Transform)>,
) {
    for (moon, mut transform) in &mut moons {
        let is_active = selection.hovered == Some(moon.0) || selection.focused == Some(moon.0);
        let target = Vec3::splat(moon_uniform_scale(moon.0) * active_scale_factor(is_active));
        transform.scale = damp_towards(
            transform.scale,
            target,
            SCALE_DAMPING_RATE,
            time.delta_secs(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;

    /// Testa que clicar numa lua disponível, sem nenhuma em foco, dá foco nela.
    #[test]
    fn clicking_available_moon_focuses_it() {
        assert_eq!(
            resolve_click(moon_info(MoonId::Europa), &MenuSelection::default()),
            ClickOutcome::Focus(MoonId::Europa)
        );
    }

    /// Testa que clicar na lua que já está em foco não muda nada.
    #[test]
    fn clicking_the_focused_moon_does_nothing() {
        let selection = MenuSelection {
            hovered: Some(MoonId::Europa),
            focused: Some(MoonId::Europa),
        };
        assert_eq!(
            resolve_click(moon_info(MoonId::Europa), &selection),
            ClickOutcome::Ignore
        );
    }

    /// Testa que clicar numa lua indisponível nunca dá foco nela, mesmo com
    /// outra lua em foco, e só pede o aviso de não implementada.
    #[test]
    fn clicking_unavailable_moon_never_focuses_it_even_while_another_is_focused() {
        let selection = MenuSelection {
            hovered: None,
            focused: Some(MoonId::Europa),
        };
        assert_eq!(
            resolve_click(moon_info(MoonId::Io), &selection),
            ClickOutcome::NotImplemented(MoonId::Io)
        );
    }

    /// Testa que o picking de malhas só passa a exigir marcadores após
    /// entrar em `Running`, e segue sem restrição enquanto o menu está ativo.
    #[test]
    fn mesh_picking_requires_markers_only_after_entering_running() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<MeshPickingSettings>()
            .add_systems(
                OnEnter(AppState::Running),
                restrict_mesh_picking_to_marked_cameras,
            );

        app.update();
        assert!(
            !app.world()
                .resource::<MeshPickingSettings>()
                .require_markers
        );

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Running);
        app.update();
        app.update();
        assert!(
            app.world()
                .resource::<MeshPickingSettings>()
                .require_markers
        );
    }
}
