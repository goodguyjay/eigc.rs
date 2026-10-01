//! Ponto de interesse e escala de tela em torno dos quais o LOD é decidido.

use bevy::prelude::{Resource, Vec3};

/// Escala de tela usada enquanto nenhuma câmera com viewport conhecido publicou a real
/// (1080 px de altura, FOV vertical de 60 graus).
pub const DEFAULT_SCREEN_SCALE: f32 = 935.3;

/// Posição e escala de tela usadas para escolher o nível de LOD.
///
/// Quem controla a câmera escreve, quem precisa de LOD lê. Assim o crate de terreno não precisa
/// depender do crate de cena.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LodFocus {
    /// Posição no mundo, em metros.
    pub position: Vec3,
    /// Pixels que um metro ocupa a 1 m de distância da câmera (ver [`screen_scale_from_fov`]).
    /// Um erro de `e` metros a `d` metros de distância ocupa `e * screen_scale / d` pixels.
    pub screen_scale: f32,
}

impl Default for LodFocus {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            screen_scale: DEFAULT_SCREEN_SCALE,
        }
    }
}

/// Calcula a escala de tela de uma câmera em perspectiva: `altura / (2 * tan(fov / 2))`.
///
/// `vertical_fov_rad` é o FOV vertical em radianos e `viewport_height_px` a altura da área
/// renderizada em pixels físicos.
pub fn screen_scale_from_fov(vertical_fov_rad: f32, viewport_height_px: f32) -> f32 {
    viewport_height_px / (2.0 * (vertical_fov_rad * 0.5).tan())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A escala padrão corresponde a 1080 px de altura e FOV vertical de 60 graus.
    #[test]
    fn default_screen_scale_matches_1080p_and_60_degree_fov() {
        let scale = screen_scale_from_fov(std::f32::consts::FRAC_PI_3, 1080.0);
        assert!((scale - DEFAULT_SCREEN_SCALE).abs() < 0.5);
    }

    /// Janela mais alta ou FOV mais estreito deixam o mesmo objeto maior na tela.
    #[test]
    fn screen_scale_grows_with_viewport_height_and_shrinks_with_fov() {
        let base = screen_scale_from_fov(std::f32::consts::FRAC_PI_3, 1080.0);
        assert!(screen_scale_from_fov(std::f32::consts::FRAC_PI_3, 2160.0) > base);
        assert!(screen_scale_from_fov(std::f32::consts::FRAC_PI_6, 1080.0) > base);
    }
}
