//! Cãmera de voo livre ("freefly") usada para navegar pela cena.

use crate::sky::{SkySettings, SkyState};
use bevy::app::App;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::{
    ButtonInput, Camera, Camera3d, Commands, Component, EulerRot, IntoScheduleConfigs, KeyCode,
    MessageReader, MouseButton, OnEnter, PerspectiveProjection, Plugin, Projection, Quat, Query,
    Res, ResMut, Resource, Single, Time, Transform, Update, Vec3, With, default, in_state,
    resource_exists,
};
use eigc_common::lod_focus::{LodFocus, screen_scale_from_fov};
use eigc_moons::AppState;
use bevy::window::{CursorGrabMode, CursorOptions};

/// Marca a câmera de voo livre e guarda o estado de orientação e os parâmetros de movimento.
#[derive(Component)]
pub struct FreeFlyCamera {
    /// Rotação horizontal acumulada, em radianos.
    pub yaw: f32,
    /// Rotação vertical acumulada, em radianos.
    pub pitch: f32,
    /// Velocidade de movimento da câmera, unidades por seg.
    pub speed: f32,
    /// Multiplicador de velocidade apertando shift
    pub sprint_multiplier: f32,
    /// Sensibilidade do mouse, aplicada ao delta de movimento do mouse
    pub sensitivity: f32,
}

impl Default for FreeFlyCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 20.0,
            sprint_multiplier: 100.0,
            sensitivity: 0.02,
        }
    }
}

/// Limite de pitch, em radianos, para não permitir que a câmera vire de cabeça para baixo.
const PITCH_LIMIT: f32 = 1.54;

/// Registra o plugin de câmera de voo livre.
pub struct FreeFlyCameraPlugin;

impl Plugin for FreeFlyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamLock>()
            .init_resource::<LodFocus>()
            .add_systems(OnEnter(AppState::Running), spawn_free_fly_camera)
            .add_systems(
                Update,
                (
                    toggle_lock,
                    mouse_look,
                    keyboard_movement,
                    lock_aim_update.run_if(resource_exists::<SkySettings>),
                    cursor_release,
                    publish_lod_focus,
                    recapture_cursor_on_click,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

/// Guarda o estado de bloqueio da câmera, se está livre ou travada em algum corpo celeste.
#[derive(Resource, Default)]
struct CamLock {
    mode: LockMode,
}

/// Modos de bloqueio da câmera.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum LockMode {
    #[default]
    Free,
    Sun,
    Jupiter,
}

/// Spawna a câmera de voo livre com projeção perspectiva e captura o cursor imediatamente.
/// Roda em `OnEnter(Running)`, para que o cursor fique livre enquanto o menu está aberto.
fn spawn_free_fly_camera(mut commands: Commands, mut cursor_options: Single<&mut CursorOptions>) {
    let translation = Vec3::new(0.0, 600.0, 1200.0);
    let mut transform = Transform::from_translation(translation);
    transform.look_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y);
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
        FreeFlyCamera {
            yaw,
            pitch,
            ..default()
        },
    ));

    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

/// Aplica o movimento do mouse à orientação da câmera, com pitch limitado para evitar inversão.
fn mouse_look(
    mut mouse_motion: MessageReader<MouseMotion>,
    mut cameras: Query<(&mut Transform, &mut FreeFlyCamera)>,
    cursor_options: Single<&CursorOptions>,
) {
    if cursor_options.grab_mode != CursorGrabMode::Locked {
        mouse_motion.clear();
        return;
    }
    let Ok((mut transform, mut camera)) = cameras.single_mut() else {
        return;
    };

    let mut delta = Vec3::ZERO;
    for motion in mouse_motion.read() {
        delta.x += motion.delta.x;
        delta.y += motion.delta.y;
    }

    if delta.x == 0.0 && delta.y == 0.0 {
        return;
    }

    camera.yaw -= delta.x * camera.sensitivity;
    camera.pitch = clamp_pitch(camera.pitch - delta.y * camera.sensitivity);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
}

/// Restringe o pitch ao intervalo `[-PITCH_LIMIT, PITCH_LIMIT]`.
///
/// Tá como teste unitário para permitir teste unitário sem depender do ECS.
fn clamp_pitch(pitch: f32) -> f32 {
    pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT)
}

/// Move a câmera segundo as teclas wasd, espaço/ctrl para subir/descer e shit para sprint.
fn keyboard_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut Transform, &FreeFlyCamera)>,
) {
    let Ok((mut transform, camera)) = cameras.single_mut() else {
        return;
    };

    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction += *transform.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= *transform.forward();
    }
    if keys.pressed(KeyCode::KeyA) {
        direction += *transform.left();
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += *transform.right();
    }
    if keys.pressed(KeyCode::Space) {
        direction += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        direction -= Vec3::Y;
    }

    if direction.length_squared() == 0.0 {
        return;
    }

    let multiplier = if keys.pressed(KeyCode::ShiftLeft) {
        camera.sprint_multiplier
    } else {
        1.0
    };

    transform.translation += direction.normalize() * camera.speed * multiplier * time.delta_secs();
}

/// Libera o cursor ao pressionar ESC.
fn cursor_release(keys: Res<ButtonInput<KeyCode>>, mut cursor_options: Single<&mut CursorOptions>) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

/// Recaptura o cursor ao clicar com o botão esquerdo, permitindo retomar o controle da câmera
/// depois que o ESC libera o cursor, sem precisar fechar e reabrir a aplicação.
fn recapture_cursor_on_click(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    if cursor_options.grab_mode == CursorGrabMode::Locked {
        return;
    }

    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

/// Atualiza a orientação da câmera para mirar no Sol ou em Júpiter, dependendo do modo de bloqueio.
fn lock_aim_update(
    lock: Res<CamLock>,
    state: Res<SkyState>,
    mut cameras: Query<(&mut Transform, &mut FreeFlyCamera)>,
) {
    let Some(forward) = resolve_lock_direction(lock.mode, &state) else {
        return;
    };

    let Ok((mut transform, mut camera)) = cameras.single_mut() else {
        return;
    };

    transform.look_to(forward, Vec3::Y);

    let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
    camera.yaw = yaw;
    camera.pitch = pitch;
}

/// Publica em `LodFocus` a posição da câmera e a escala de tela (FOV e altura da janela), para o
/// terreno escolher o nível de LOD por erro em pixels.
///
/// Lê o `Transform` e não o `GlobalTransform`, que só reflete o movimento do frame seguinte.
/// Enquanto o viewport da câmera não é conhecido, mantém a escala de tela anterior.
fn publish_lod_focus(
    camera: Single<(&Transform, &Projection, &Camera), With<FreeFlyCamera>>,
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

/// Alterna o modo de bloqueio da câmera entre livre, travada no Sol ou travada em Júpiter.
fn toggle_lock(keys: Res<ButtonInput<KeyCode>>, mut lock: ResMut<CamLock>) {
    if keys.just_pressed(KeyCode::KeyF) {
        lock.mode = match lock.mode {
            LockMode::Sun => LockMode::Free,
            _ => LockMode::Sun,
        };
    }

    if keys.just_pressed(KeyCode::KeyJ) {
        lock.mode = match lock.mode {
            LockMode::Jupiter => LockMode::Free,
            _ => LockMode::Jupiter,
        };
    }
}

/// Resolve a direção alvo do lock atual a partir das configurações do céu.
///
/// Retorna `None` quando o modo é `Free`, já que não há direção associada.
fn resolve_lock_direction(mode: LockMode, state: &SkyState) -> Option<Vec3> {
    match mode {
        LockMode::Free => None,
        LockMode::Sun => Some(state.sun_dir.normalize()),
        LockMode::Jupiter => Some(state.jupiter_dir.normalize()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimePlugin;

    /// Testa que o pitch é corretamente limitado dentro dos limites definidos.
    #[test]
    fn clamp_pitch_restricts_pitch_within_limits() {
        let pitch = 2.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, PITCH_LIMIT);

        let pitch = -2.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, -PITCH_LIMIT);

        let pitch = 1.0;
        let clamped_pitch = clamp_pitch(pitch);
        assert_eq!(clamped_pitch, pitch);
    }

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
            FreeFlyCamera::default(),
        ));
        let default_scale = app.world().resource::<LodFocus>().screen_scale;

        app.update();

        let focus = app.world().resource::<LodFocus>();
        assert_eq!(focus.position, position);
        assert_eq!(focus.screen_scale, default_scale);
    }

    /// Testa que segurar W descola a câmera para frente ao longo do tempo
    #[test]
    fn keyboard_movement_moves_camera_forward_when_w_pressed() {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, keyboard_movement);

        let transform = Transform::from_translation(Vec3::ZERO);
        let camera = FreeFlyCamera::default();
        let entity = app.world_mut().spawn((transform, camera)).id();

        app.update();
        let position_after_first_update = app.world().get::<Transform>(entity).unwrap().translation;

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        app.update();
        let position_after_second_update =
            app.world().get::<Transform>(entity).unwrap().translation;

        assert!(
            position_after_second_update.z < position_after_first_update.z,
            "câmera deveria se descolar para frente (-Z) ao segurar W, mas foi de {:?} para {:?}",
            position_after_first_update,
            position_after_second_update
        );
    }

    /// Testa que, depois do cursor ser liberado (ESC), um clique esquerdo o recaptura —
    /// sem isso, a única forma de recuperar o controle da câmera era reiniciar o app.
    #[test]
    fn left_click_recaptures_cursor_after_it_was_released() {
        let mut app = App::new();

        app.init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, recapture_cursor_on_click);

        let entity = app
            .world_mut()
            .spawn(CursorOptions {
                visible: true,
                grab_mode: CursorGrabMode::None,
                ..default()
            })
            .id();

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();

        let cursor_options = app.world().get::<CursorOptions>(entity).unwrap();
        assert_eq!(cursor_options.grab_mode, CursorGrabMode::Locked);
        assert!(!cursor_options.visible);
    }

    /// Testa que `resolve_lock_direction` retorna a direção correta pra cada
    /// modo de lock, e `None` quando livre.
    #[test]
    fn resolve_lock_direction_returns_body_direction() {
        let state = SkyState {
            sun_dir: Vec3::new(1.0, 0.0, 0.0),
            jupiter_dir: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        };

        assert_eq!(resolve_lock_direction(LockMode::Free, &state), None);

        assert_eq!(
            resolve_lock_direction(LockMode::Sun, &state),
            Some(Vec3::new(1.0, 0.0, 0.0))
        );

        assert_eq!(
            resolve_lock_direction(LockMode::Jupiter, &state),
            Some(Vec3::new(0.0, 0.0, 1.0))
        );
    }

    /// Testa que, com o lock travado no Sol, a câmera acaba orientada na direção
    /// de `base_sun_dir` após um update, e que yaw/pitch ficam sincronizados
    /// com essa orientação.
    #[test]
    fn lock_aim_update_orients_camera_toward_sun_when_locked() {
        let mut app = App::new();

        let sun_dir = Vec3::new(1.0, 0.0, 0.0).normalize();
        let jupiter_dir = Vec3::new(0.0, 0.0, 1.0).normalize();

        app.insert_resource(CamLock {
            mode: LockMode::Sun,
        })
        .insert_resource(SkyState {
            sun_dir,
            jupiter_dir,
            ..default()
        })
        .add_systems(Update, lock_aim_update);

        // câmera arbitrária, olhando pra uma direção que não é nem sol nem
        // júpiter, com yaw/pitch velhos que não deviam sobreviver ao lock.
        let transform = Transform::from_translation(Vec3::ZERO);
        let camera = FreeFlyCamera {
            yaw: 2.5,
            pitch: 0.5,
            ..default()
        };
        let entity = app.world_mut().spawn((transform, camera)).id();

        app.update();

        let updated_transform = app.world().get::<Transform>(entity).unwrap();
        let updated_camera = app.world().get::<FreeFlyCamera>(entity).unwrap();

        let mut expected_transform = Transform::from_translation(Vec3::ZERO);
        expected_transform.look_to(sun_dir, Vec3::Y);
        let (expected_yaw, expected_pitch, _roll) =
            expected_transform.rotation.to_euler(EulerRot::YXZ);

        assert!(
            updated_transform
                .rotation
                .abs_diff_eq(expected_transform.rotation, 1e-5),
            "câmera deveria olhar na direção do sol, mas rotação foi {:?}, esperado {:?}",
            updated_transform.rotation,
            expected_transform.rotation
        );

        assert!(
            (updated_camera.yaw - expected_yaw).abs() < 1e-5,
            "yaw deveria estar sincronizado com a direção travada, foi {}, esperado {}",
            updated_camera.yaw,
            expected_yaw
        );
        assert!(
            (updated_camera.pitch - expected_pitch).abs() < 1e-5,
            "pitch deveria estar sincronizado com a direção travada, foi {}, esperado {}",
            updated_camera.pitch,
            expected_pitch
        );
    }

    /// Testa que, com o lock livre, `lock_aim_update` não altera a orientação
    /// da câmera nem os campos yaw/pitch.
    #[test]
    fn lock_aim_update_does_nothing_when_free() {
        let mut app = App::new();

        app.insert_resource(CamLock {
            mode: LockMode::Free,
        })
        .insert_resource(SkyState {
            sun_dir: Vec3::new(1.0, 0.0, 0.0),
            jupiter_dir: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        })
        .add_systems(Update, lock_aim_update);

        let transform = Transform::from_translation(Vec3::ZERO);
        let camera = FreeFlyCamera {
            yaw: 1.2,
            pitch: 0.3,
            ..default()
        };
        let entity = app.world_mut().spawn((transform, camera)).id();

        app.update();

        let updated_transform = app.world().get::<Transform>(entity).unwrap();
        let updated_camera = app.world().get::<FreeFlyCamera>(entity).unwrap();

        assert_eq!(updated_transform.rotation, Quat::IDENTITY);
        assert_eq!(updated_camera.yaw, 1.2);
        assert_eq!(updated_camera.pitch, 0.3);
    }
}
