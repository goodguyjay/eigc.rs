//! Limite horizontal da câmera: impede sair do recorte do terreno. O anel de extensão além da
//! borda (`eigc_terrain::far_field`) é só visual, então nenhum modo de movimento pode chegar
//! nele. A altitude não é limitada.

use crate::camera::first_person::FirstPersonCamera;
use bevy::prelude::{Query, Res, Transform, Vec3, With};
use eigc_terrain::params::TerrainParams;

/// Distância, em metros, que a câmera mantém da borda do recorte.
const PLAY_AREA_MARGIN_M: f32 = 50.0;

/// Prende `x` e `z` dentro de `[-half_extent_m, half_extent_m]`, eixo a eixo, sem mexer no `y`.
/// Eixo a eixo, quem anda contra a borda desliza ao longo dela em vez de parar.
fn clamp_to_play_area(position: Vec3, half_extent_m: f32) -> Vec3 {
    let limit = half_extent_m.max(0.0);
    Vec3::new(
        position.x.clamp(-limit, limit),
        position.y,
        position.z.clamp(-limit, limit),
    )
}

/// Mantém a câmera dentro do recorte, a `PLAY_AREA_MARGIN_M` da borda. Roda depois dos sistemas
/// de movimento e antes de `follow_ground`, para o chão ser lido já na posição limitada.
///
/// Precisa de `TerrainParams`, que o terreno insere ao entrar em `Running`: quem registra este
/// sistema deve usar `run_if(resource_exists::<TerrainParams>)`.
pub(super) fn keep_inside_play_area(
    params: Res<TerrainParams>,
    mut cameras: Query<&mut Transform, With<FirstPersonCamera>>,
) {
    let half_extent = params.size * 0.5 - PLAY_AREA_MARGIN_M;
    for mut transform in &mut cameras {
        let clamped = clamp_to_play_area(transform.translation, half_extent);
        if clamped != transform.translation {
            transform.translation = clamped;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Update, Vec2};
    use eigc_terrain::params::TerrainShape;

    /// Testa que dentro do recorte a posição não muda.
    #[test]
    fn clamp_to_play_area_keeps_positions_inside() {
        let inside = Vec3::new(-10_000.0, 35.0, 10_949.0);
        assert_eq!(clamp_to_play_area(inside, 10_950.0), inside);
    }

    /// Testa que fora do recorte só o eixo que passou da borda é preso (desliza ao longo dela) e
    /// que a altitude nunca é limitada.
    #[test]
    fn clamp_to_play_area_clamps_each_axis_and_leaves_altitude_free() {
        let clamped = clamp_to_play_area(Vec3::new(12_000.0, 4_000.0, -300.0), 10_950.0);
        assert_eq!(clamped, Vec3::new(10_950.0, 4_000.0, -300.0));

        let corner = clamp_to_play_area(Vec3::new(-20_000.0, -5.0, 30_000.0), 10_950.0);
        assert_eq!(corner, Vec3::new(-10_950.0, -5.0, 10_950.0));
    }

    /// Testa que o sistema traz de volta uma câmera que passou da borda, com a margem aplicada.
    #[test]
    fn keep_inside_play_area_pulls_the_camera_back_inside_the_cutout() {
        let mut app = App::new();
        app.insert_resource(TerrainParams {
            size: 22_000.0,
            res: 2,
            freq: 1.0,
            line_dir: Vec2::X,
            seed: 0,
            vertical_exaggeration: 1.0,
            shape: TerrainShape::Square,
        })
        .add_systems(Update, keep_inside_play_area);
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(11_500.0, 900.0, 0.0),
                FirstPersonCamera::default(),
            ))
            .id();

        app.update();

        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert_eq!(position, Vec3::new(10_950.0, 900.0, 0.0));
    }
}
