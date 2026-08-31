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
}
