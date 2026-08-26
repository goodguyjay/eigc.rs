use bevy::prelude::{Resource, Vec2};

/// Parâmetros de geração de terreno
#[derive(Resource, Clone, Copy)]
pub struct TerrainParams {
    /// Tamanho do mundo em metros
    pub size: f32,
    /// Resolução da grade (N x N vértices)
    pub res: u32,
    /// Escala vertical (metros)
    pub amp: f32,
    /// Frequência base (menor → características mais amplas)
    pub freq: f32,
    /// Direção das "lineae" (normalizada)
    pub line_dir: Vec2,
    /// Semente do gerador de números aleatórios
    pub seed: u32,
}
