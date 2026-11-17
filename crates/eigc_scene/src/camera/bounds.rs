//! Limite da área jogável da câmera em primeira pessoa: o jogador anda dentro de um disco
//! centrado na origem do terreno.

use crate::camera::first_person::FirstPersonCamera;
use bevy::prelude::{Query, Res, Transform, Vec2, With};
use eigc_terrain::params::{TerrainParams, TerrainShape};
use eigc_terrain::recipe::playable_radius_m;

/// Projeta `xz` de volta na circunferência de raio `radius` quando ele a ultrapassa. Dentro do
/// raio devolve o próprio `xz`. Projetar, em vez de parar o movimento, deixa o jogador deslizar
/// pela borda.
///
/// Com `radius` negativo o raio vale zero. O resultado pode passar do raio só pelo erro de ponto
/// flutuante da divisão.
pub fn clamp_to_disc(xz: Vec2, radius: f32) -> Vec2 {
    let radius = radius.max(0.0);
    let length = xz.length();
    if length <= radius {
        xz
    } else {
        xz * (radius / length)
    }
}

/// Condição de sistema do limite em disco: falso só quando o terreno carregado é quadrado (Io,
/// que é limitado pelo recorte em `play_area`). `playable_radius_m()` é o raio de Europa e não
/// vale para um recorte quadrado.
pub(super) fn terrain_is_not_square(params: Option<Res<TerrainParams>>) -> bool {
    params.is_none_or(|params| params.shape != TerrainShape::Square)
}

/// Mantém a câmera em primeira pessoa dentro de `playable_radius_m()` do centro do terreno,
/// mexendo só em `x` e `z` de `Transform.translation`. O `y` é de `follow_ground`.
///
/// Precisa rodar depois de `walk_movement` e antes de `follow_ground` e de `publish_lod_focus`,
/// para a altura do olho e o foco do LOD usarem a posição já corrigida. Quem registra deve usar
/// `run_if(in_mode(MovementMode::Walk))`: o voo livre de debug fica solto.
pub(super) fn limit_to_playable_area(mut cameras: Query<&mut Transform, With<FirstPersonCamera>>) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };

    let position = Vec2::new(transform.translation.x, transform.translation.z);
    let clamped = clamp_to_disc(position, playable_radius_m());
    if clamped != position {
        transform.translation.x = clamped.x;
        transform.translation.z = clamped.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Entity, Update, Vec3};
    use proptest::{prop_assert, prop_assert_eq, prop_assume, proptest};

    /// Testa que um ponto dentro do raio volta igual, inclusive a origem e o ponto na borda.
    #[test]
    fn clamp_to_disc_keeps_points_inside_the_radius() {
        for point in [Vec2::ZERO, Vec2::new(3.0, -4.0), Vec2::new(0.0, 10.0)] {
            assert_eq!(clamp_to_disc(point, 10.0), point);
        }
    }

    /// Testa que um ponto fora do raio vai para a circunferência, na mesma direção.
    #[test]
    fn clamp_to_disc_projects_outside_points_onto_the_circle() {
        let clamped = clamp_to_disc(Vec2::new(30.0, -40.0), 10.0);
        assert!(
            clamped.abs_diff_eq(Vec2::new(6.0, -8.0), 1e-5),
            "{clamped:?}"
        );
    }

    /// Testa que um raio negativo se comporta como raio zero.
    #[test]
    fn clamp_to_disc_treats_a_negative_radius_as_zero() {
        assert_eq!(clamp_to_disc(Vec2::new(5.0, 5.0), -3.0), Vec2::ZERO);
    }

    /// Câmera em primeira pessoa em `position`, num app só com o sistema de limite.
    fn app_with_camera_at(position: Vec3) -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, limit_to_playable_area);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_translation(position),
                FirstPersonCamera::default(),
            ))
            .id();
        (app, entity)
    }

    /// Testa que o sistema puxa a câmera de fora para a borda jogável sem mexer no `y`.
    #[test]
    fn system_pulls_a_camera_outside_the_playable_radius_back_to_the_edge() {
        let radius = playable_radius_m();
        let (mut app, entity) = app_with_camera_at(Vec3::new(radius * 2.0, 7.0, 0.0));

        app.update();

        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert!((position.x - radius).abs() < radius * 1e-5, "{position:?}");
        assert_eq!((position.y, position.z), (7.0, 0.0));
    }

    /// Testa que o sistema não mexe numa câmera que já está dentro do raio.
    #[test]
    fn system_leaves_a_camera_inside_the_playable_radius_alone() {
        let (mut app, entity) = app_with_camera_at(Vec3::new(120.0, 3.0, -80.0));

        app.update();

        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert_eq!(position, Vec3::new(120.0, 3.0, -80.0));
    }

    proptest! {
        /// A saída nunca passa do raio (a menos do erro de ponto flutuante).
        #[test]
        fn clamp_to_disc_output_stays_inside_the_radius(
            x in -50_000.0_f32..50_000.0,
            z in -50_000.0_f32..50_000.0,
            radius in 0.0_f32..20_000.0,
        ) {
            let clamped = clamp_to_disc(Vec2::new(x, z), radius);
            prop_assert!(clamped.length() <= radius * (1.0 + 1e-5) + 1e-3);
        }

        /// Um ponto dentro do raio sai idêntico.
        #[test]
        fn clamp_to_disc_leaves_inside_points_untouched(
            angle in 0.0_f32..std::f32::consts::TAU,
            fraction in 0.0_f32..=1.0,
            radius in 1.0_f32..20_000.0,
        ) {
            let point = Vec2::from_angle(angle) * (radius * fraction);
            prop_assume!(point.length() <= radius);
            prop_assert_eq!(clamp_to_disc(point, radius), point);
        }

        /// Aplicar o limite duas vezes dá o mesmo que uma (a menos do erro de ponto flutuante).
        #[test]
        fn clamp_to_disc_is_idempotent(
            x in -50_000.0_f32..50_000.0,
            z in -50_000.0_f32..50_000.0,
            radius in 0.0_f32..20_000.0,
        ) {
            let once = clamp_to_disc(Vec2::new(x, z), radius);
            let twice = clamp_to_disc(once, radius);
            prop_assert!(once.abs_diff_eq(twice, radius * 1e-5 + 1e-3));
        }

        /// A projeção mantém a direção do ponto: o ponto e a saída são paralelos.
        #[test]
        fn clamp_to_disc_keeps_the_direction(
            angle in 0.0_f32..std::f32::consts::TAU,
            distance in 0.0_f32..50_000.0,
            radius in 1.0_f32..20_000.0,
        ) {
            let direction = Vec2::from_angle(angle);
            let clamped = clamp_to_disc(direction * distance, radius);
            prop_assert!(clamped.length() < 1e-3 || clamped.normalize().abs_diff_eq(direction, 1e-4));
        }
    }
}
