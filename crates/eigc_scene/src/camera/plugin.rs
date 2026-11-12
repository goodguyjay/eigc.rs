//! Plugin da câmera: spawn, ordem dos sistemas de orientação e movimento, e publicação do foco
//! de LOD.

use crate::camera::first_person::{FirstPersonCamera, follow_ground, walk_movement};
use crate::camera::free_fly_camera::{FreeFlyCamera, free_fly_movement};
use crate::camera::look::{
    CamLock, CameraLook, capture_cursor, lock_aim_update, mouse_look, recapture_cursor_on_click,
    release_cursor, toggle_lock,
};
#[cfg(feature = "dev")]
use crate::camera::mode::toggle_movement_mode;
use crate::camera::mode::{MovementMode, in_mode};
use crate::sky::SkySettings;
use bevy::app::App;
use bevy::prelude::{
    Camera, Camera3d, Commands, EulerRot, IntoScheduleConfigs, OnEnter, OnExit,
    PerspectiveProjection, Plugin, Projection, ResMut, Single, Transform, Update, Vec3, With,
    default, in_state, not, resource_exists,
};
use bevy::state::state_scoped::DespawnOnExit;
#[cfg(feature = "dev")]
use bevy::prelude::{Res, Resource};
use bevy::window::{CursorGrabMode, CursorOptions};
use eigc_common::lod_focus::{LodFocus, screen_scale_from_fov};
use eigc_moons::{AppState, PauseState};
use eigc_terrain::pipeline::HeightResource;

/// Registra a câmera: primeira pessoa por padrão e, com a feature `dev`, voo livre alternável
/// com Tab. Com `PauseState::Paused` os sistemas de câmera ficam parados e o cursor livre; ao sair
/// de `Running` a câmera é despawnada e o estado da sessão volta ao padrão.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamLock>()
            .init_resource::<MovementMode>()
            .init_resource::<LodFocus>()
            .add_systems(OnEnter(AppState::Running), spawn_camera)
            .add_systems(OnEnter(PauseState::Playing), capture_cursor)
            .add_systems(OnEnter(PauseState::Paused), release_cursor)
            .add_systems(
                OnExit(AppState::Running),
                (reset_camera_session, release_cursor),
            )
            .add_systems(
                Update,
                (
                    toggle_lock,
                    mouse_look,
                    free_fly_movement.run_if(in_mode(MovementMode::FreeFly)),
                    walk_movement.run_if(in_mode(MovementMode::Walk)),
                    follow_ground
                        .run_if(in_mode(MovementMode::Walk))
                        .run_if(resource_exists::<HeightResource>),
                    lock_aim_update.run_if(resource_exists::<SkySettings>),
                    publish_lod_focus,
                    recapture_cursor_on_click,
                )
                    .chain()
                    .run_if(in_state(AppState::Running))
                    .run_if(not(in_state(PauseState::Paused))),
            );

        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            toggle_movement_mode
                .before(mouse_look)
                .run_if(in_state(AppState::Running))
                .run_if(not(in_state(PauseState::Paused))),
        );
    }
}

/// Posição de spawn da câmera: no centro da clareira plana de Europa, a ~1,75 m do chão. Em
/// primeira pessoa o `y` é corrigido no primeiro quadro por `follow_ground`.
const SPAWN_POSITION: Vec3 = Vec3::new(0.0, 0.3, 0.0);

/// Ponto para onde a câmera olha ao nascer: horizonte à frente, com leve inclinação para baixo
/// (~6 graus).
const SPAWN_LOOK_TARGET: Vec3 = Vec3::new(0.0, -6.0, -60.0);

/// Posição de spawn da câmera forçada por quem monta o app (flag de debug `--spawn`). Só existe
/// com a feature `dev`. Quando presente, a câmera nasce em voo livre: em primeira pessoa o `y`
/// seria sobrescrito no primeiro quadro por `follow_ground`.
#[cfg(feature = "dev")]
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DebugSpawnPosition(pub Vec3);

/// Spawna a câmera com projeção perspectiva e captura o cursor imediatamente. Roda em
/// `OnEnter(Running)`, para que o cursor fique livre enquanto o menu está aberto.
///
/// A entidade carrega os parâmetros dos dois modos de movimento; quem decide qual vale é
/// [`MovementMode`].
fn spawn_camera(
    mut commands: Commands,
    mut cursor_options: Single<&mut CursorOptions>,
    #[cfg(feature = "dev")] debug_spawn: Option<Res<DebugSpawnPosition>>,
    #[cfg(feature = "dev")] mut movement_mode: ResMut<MovementMode>,
) {
    let position = SPAWN_POSITION;
    #[cfg(feature = "dev")]
    let position = match debug_spawn {
        Some(debug_spawn) => {
            *movement_mode = MovementMode::FreeFly;
            debug_spawn.0
        }
        None => position,
    };

    let mut transform = Transform::from_translation(position);
    transform.look_at(SPAWN_LOOK_TARGET, Vec3::Y);
    let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);

    commands.spawn((
        Camera3d::default(),
        transform,
        Projection::Perspective(PerspectiveProjection {
            near: 0.1,
            far: 50_000.0,
            fov: std::f32::consts::FRAC_PI_3,
            ..default()
        }),
        FirstPersonCamera::default(),
        FreeFlyCamera::default(),
        CameraLook {
            yaw,
            pitch,
            ..default()
        },
        DespawnOnExit(AppState::Running),
    ));

    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

/// Devolve trava de mira, modo de movimento e foco de LOD aos valores iniciais ao sair de
/// `Running`, para a próxima exploração começar do zero em vez de herdar a sessão anterior.
fn reset_camera_session(mut commands: Commands) {
    commands.insert_resource(CamLock::default());
    commands.insert_resource(MovementMode::default());
    commands.insert_resource(LodFocus::default());
}

/// Publica em `LodFocus` a posição da câmera e a escala de tela (FOV e altura da janela), para o
/// terreno escolher o nível de LOD por erro em pixels.
///
/// Lê o `Transform` e não o `GlobalTransform`, que só reflete o movimento do frame seguinte.
/// Enquanto o viewport da câmera não é conhecido, mantém a escala de tela anterior.
fn publish_lod_focus(
    camera: Single<(&Transform, &Projection, &Camera), With<CameraLook>>,
    mut focus: ResMut<LodFocus>,
) {
    let (transform, projection, camera) = *camera;
    focus.position = transform.translation;

    if let (Projection::Perspective(perspective), Some(viewport)) =
        (projection, camera.physical_viewport_size())
    {
        focus.screen_scale = screen_scale_from_fov(perspective.fov, viewport.y as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que a posição atual da câmera aparece em `LodFocus` depois de um update, e que a
    /// escala de tela padrão é mantida enquanto a câmera não tem viewport conhecido.
    #[test]
    fn publish_lod_focus_copies_camera_position_and_keeps_screen_scale_without_viewport() {
        let mut app = App::new();
        app.init_resource::<LodFocus>()
            .add_systems(Update, publish_lod_focus);

        let position = Vec3::new(120.0, 600.0, -35.0);
        app.world_mut().spawn((
            Transform::from_translation(position),
            Camera::default(),
            Projection::default(),
            CameraLook::default(),
        ));
        let default_scale = app.world().resource::<LodFocus>().screen_scale;

        app.update();

        let focus = app.world().resource::<LodFocus>();
        assert_eq!(focus.position, position);
        assert_eq!(focus.screen_scale, default_scale);
    }
}
