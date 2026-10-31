//! UI por cima da cena 3D da abertura: a luz central, o véu claro e o clarão final da
//! transição para a cena principal.

use bevy::prelude::{
    Alpha, Assets, BackgroundColor, Color, Commands, Component, Entity, GlobalZIndex, Image,
    ImageNode, Node, Pickable, PositionType, Query, Res, ResMut, State, UiTargetCamera, Val, Vec2,
    With, Without, default,
};
use bevy::window::{PrimaryWindow, Window};

use super::{IntroCamera, IntroClock, IntroEntity, IntroState, smoothstep, texture};

/// Peso da luz central (núcleo, halo e véu) em relação às intensidades descritas na
/// especificação. `0.6` é a opção "leve" escolhida para a abertura.
const CENTRAL_LIGHT_INTENSITY: f32 = 0.6;

const CORE_COLOR: Color = Color::srgb(1.0, 0.94, 0.84);
const HALO_COLOR: Color = Color::srgb(1.0, 0.78, 0.59);
const VEIL_COLOR: Color = Color::srgb(1.0, 0.97, 0.91);
const FLASH_COLOR: Color = Color::srgb(1.0, 0.96, 0.89);

/// Acima de qualquer UI que já exista por baixo (ex.: o menu), para o clarão final cobrir a tela
/// inteira durante a transição.
const OVERLAY_Z_INDEX: i32 = 1000;

/// Núcleo brilhante da luz central, no centro da tela.
#[derive(Component)]
pub(super) struct CoreGlow;

/// Halo mais fraco e maior ao redor do núcleo da luz central.
#[derive(Component)]
pub(super) struct HaloGlow;

/// Véu claro em tela cheia que acompanha o crescimento da luz central.
#[derive(Component)]
pub(super) struct LightVeil;

/// Clarão final em tela cheia que revela a cena principal na transição.
#[derive(Component)]
pub(super) struct FinalFlash;

/// Monta o nó raiz da UI da abertura e seus filhos, depois de `spawn_world`.
pub(super) fn spawn_overlay(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    camera: Query<Entity, With<IntroCamera>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let glow = images.add(texture::glow_image());

    commands
        .spawn((
            IntroEntity,
            UiTargetCamera(camera),
            GlobalZIndex(OVERLAY_Z_INDEX),
            Pickable::IGNORE,
            full_screen_node(),
        ))
        .with_children(|root| {
            root.spawn((
                HaloGlow,
                Pickable::IGNORE,
                ImageNode {
                    image: glow.clone(),
                    color: HALO_COLOR.with_alpha(0.0),
                    ..default()
                },
                centered_node(),
            ));
            root.spawn((
                CoreGlow,
                Pickable::IGNORE,
                ImageNode {
                    image: glow,
                    color: CORE_COLOR.with_alpha(0.0),
                    ..default()
                },
                centered_node(),
            ));
            root.spawn((
                LightVeil,
                Pickable::IGNORE,
                BackgroundColor(VEIL_COLOR.with_alpha(0.0)),
                full_screen_node(),
            ));
            root.spawn((
                FinalFlash,
                Pickable::IGNORE,
                BackgroundColor(FLASH_COLOR.with_alpha(0.0)),
                full_screen_node(),
            ));
        });
}

fn full_screen_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

fn centered_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        ..default()
    }
}

/// Redimensiona e reposiciona um nó quadrado para ficar centralizado em `center`.
fn center_square(node: &mut Node, center: Vec2, diameter: f32) {
    node.width = Val::Px(diameter);
    node.height = Val::Px(diameter);
    node.left = Val::Px(center.x - diameter / 2.0);
    node.top = Val::Px(center.y - diameter / 2.0);
}

/// Atualiza a luz central, o véu claro e o clarão final a cada quadro, tanto em `Playing` quanto
/// em `Revealing`. Júpiter fica sempre no centro da tela, porque a câmera da abertura olha
/// direto para a origem o tempo todo.
#[allow(clippy::type_complexity)]
pub(super) fn update_overlay(
    clock: Res<IntroClock>,
    intro_state: Res<State<IntroState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut core: Query<(&mut Node, &mut ImageNode), (With<CoreGlow>, Without<HaloGlow>)>,
    mut halo: Query<(&mut Node, &mut ImageNode), (With<HaloGlow>, Without<CoreGlow>)>,
    mut veil: Query<&mut BackgroundColor, (With<LightVeil>, Without<FinalFlash>)>,
    mut flash: Query<&mut BackgroundColor, (With<FinalFlash>, Without<LightVeil>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let height = window.height();
    let center = Vec2::new(window.width(), height) / 2.0;
    let t = clock.t;

    // Assim que a cena 3D da abertura é removida (saída de `Playing`), a luz central e o véu
    // caem pra zero de propósito: o clarão final, que já está perto do pico nesse instante,
    // cobre a troca.
    let grow = if *intro_state.get() == IntroState::Playing {
        smoothstep(4.4, 6.0, t)
    } else {
        0.0
    };
    let pulse = 1.0 + 0.06 * (6.0 * t).sin();

    if let Ok((mut node, mut image)) = core.single_mut() {
        let diameter = 2.0 * height * (0.05 + 0.9 * grow * grow) * pulse;
        center_square(&mut node, center, diameter);
        image.color = CORE_COLOR.with_alpha(0.75 * grow * CENTRAL_LIGHT_INTENSITY);
    }

    if let Ok((mut node, mut image)) = halo.single_mut() {
        let diameter = 2.0 * height * (0.05 + 0.9 * grow * grow) * pulse * 2.2;
        center_square(&mut node, center, diameter);
        image.color = HALO_COLOR.with_alpha(0.25 * grow * CENTRAL_LIGHT_INTENSITY);
    }

    if let Ok(mut background) = veil.single_mut() {
        background.0 = VEIL_COLOR.with_alpha(0.35 * grow * grow * CENTRAL_LIGHT_INTENSITY);
    }

    if let Ok(mut background) = flash.single_mut() {
        let alpha = 0.95 * smoothstep(5.65, 6.0, t) * (1.0 - smoothstep(6.05, 6.7, t));
        background.0 = FLASH_COLOR.with_alpha(alpha.min(1.0));
    }
}
