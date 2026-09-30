//! Ponto de interesse em torno do qual o LOD é decidido.

use bevy::prelude::{Resource, Vec3};

/// Posição no mundo usada para escolher o nível de LOD.
///
/// Quem controla a câmera escreve, quem precisa de LOD lê. Assim o crate de terreno não precisa
/// depender do crate de cena.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct LodFocus {
    /// Posição no mundo, em metros.
    pub position: Vec3,
}
