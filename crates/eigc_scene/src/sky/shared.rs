//! Funções compartilhadas entre os corpos celestes (sol, Júpiter)

use bevy::prelude::Vec3;

/// Calcula posição e escala de um disco celeste distante
pub fn place_celestial_disc(
    cam_translation: Vec3,
    far_plane: f32,
    direction: Vec3,
    angular_diameter_deg: f32,
    sky_radius: f32,
) -> (Vec3, f32) {
    let sky_r = (far_plane * 0.85).min(sky_radius);
    let dir = direction.normalize();
    let position = cam_translation + dir * sky_r;

    let theta = angular_diameter_deg.to_radians();
    let scale = sky_r * (0.5 * theta).tan();

    (position, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa se a posição do disco celeste está na direção correta multiplicada pelo raio do céu
    #[test]
    fn disc_position_sits_at_direction_times_sky_radius() {
        let cam = Vec3::ZERO;
        let (pos, _scale) = place_celestial_disc(cam, 50_000.0, Vec3::X, 10.0, 10_000.0);
        assert!((pos - Vec3::new(10_000.0, 0.0, 0.0)).length() < 0.01);
    }

    /// Testa se o raio do céu é limitado em relação ao plano distante
    #[test]
    fn sky_radius_clamps_relative_to_far_plane() {
        let cam = Vec3::ZERO;
        let (pos, _scale) = place_celestial_disc(cam, 1_000.0, Vec3::X, 10.0, 10_000.0);
        assert!((pos.x - 850.0).abs() < 0.01);
    }

    /// Testa se o raio do céu é limitado em relação ao limite máximo
    #[test]
    fn larger_angular_diameter_yields_larger_scale() {
        let cam = Vec3::ZERO;
        let (_pos, scale_small) = place_celestial_disc(cam, 50_000.0, Vec3::X, 1.0, 10_000.0);
        let (_pos, scale_large) = place_celestial_disc(cam, 50_000.0, Vec3::X, 20.0, 10_000.0);
        assert!(scale_large > scale_small);
    }

    /// Critério de aceite da issue #8: o disco subtende exatamente o diâmetro angular pedido,
    /// para as quatro luas. Os valores da issue vêm de `2·atan(R_júpiter / distância)`, então a
    /// medida inversa correta aqui também é `2·atan(raio / sky_r)`.
    #[rstest::rstest]
    #[case::io(18.8)]
    #[case::europa(11.9)]
    #[case::ganymede(7.5)]
    #[case::callisto(4.3)]
    fn disc_subtends_requested_angular_diameter(#[case] angular_diameter_deg: f32) {
        let (pos, scale) = place_celestial_disc(
            Vec3::ZERO,
            50_000.0,
            Vec3::new(0.35, 0.25, -0.90),
            angular_diameter_deg,
            10_000.0,
        );

        let measured_deg = 2.0 * (scale / pos.length()).atan().to_degrees();
        assert!(
            (measured_deg - angular_diameter_deg).abs() < 1e-3,
            "esperado {angular_diameter_deg}°, medido {measured_deg}°"
        );
    }

    /// Critério de aceite da issue #8: a proporção de tamanho entre Europa e as outras luas
    /// segue `tan(θ_lua/2) / tan(θ_europa/2)`, independente do raio do céu.
    #[rstest::rstest]
    #[case::io(18.8)]
    #[case::ganymede(7.5)]
    #[case::callisto(4.3)]
    fn scale_ratio_against_europa_matches_angular_ratio(#[case] other_deg: f32) {
        let europa_deg = 11.9;
        let (_pos, scale_europa) =
            place_celestial_disc(Vec3::ZERO, 50_000.0, Vec3::X, europa_deg, 10_000.0);
        let (_pos, scale_other) =
            place_celestial_disc(Vec3::ZERO, 50_000.0, Vec3::X, other_deg, 10_000.0);

        let expected = (other_deg.to_radians() * 0.5).tan() / (europa_deg.to_radians() * 0.5).tan();
        let ratio = scale_other / scale_europa;
        assert!(
            (ratio - expected).abs() < 1e-4,
            "proporção esperada {expected}, obtida {ratio}"
        );
    }
}
