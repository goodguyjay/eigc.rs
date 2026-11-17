use bevy::prelude::{Resource, Vec2};

/// Formato do recorte do terreno no plano XZ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainShape {
    /// Quadrado inteiro de lado `size`. Io usa este: o anel de extensão (`far_field`) começa na
    /// borda quadrada, então tirar os cantos abriria buracos entre o recorte e o anel.
    Square,
    /// Só o círculo inscrito no quadrado: chunks totalmente fora dele não são criados (Europa).
    Disc,
}

/// Parâmetros de geração de terreno
#[derive(Resource, Clone, Copy)]
pub struct TerrainParams {
    /// Tamanho do mundo em metros
    pub size: f32,
    /// Resolução da grade (N x N vértices)
    pub res: u32,
    /// Frequência base (menor → características mais amplas)
    pub freq: f32,
    /// Direção das "lineae" (normalizada)
    pub line_dir: Vec2,
    /// Semente do gerador de números aleatórios
    pub seed: u32,
    /// Fator multiplicado sobre o relevo de origem real (DTM), para o terreno ler melhor na
    /// escala de passo humano. Licença artística consciente (a elevação real é suave demais
    /// para a câmera em primeira pessoa), no mesmo espírito do traçado em arco das lineae.
    /// `1.0` reproduz a elevação medida.
    pub vertical_exaggeration: f32,
    /// Formato do recorte: quadrado inteiro ou só o círculo inscrito.
    pub shape: TerrainShape,
}

impl TerrainParams {
    /// Raio, em metros, do círculo inscrito no recorte quadrado. Com `TerrainShape::Disc`, o
    /// terreno só tem malha dentro dele: chunks totalmente fora do círculo não são criados.
    pub fn disc_radius_m(&self) -> f32 {
        self.size * 0.5
    }

    /// Raio do disco que limita a malha, ou `None` quando o recorte é o quadrado inteiro.
    pub fn mesh_disc_radius_m(&self) -> Option<f32> {
        match self.shape {
            TerrainShape::Square => None,
            TerrainShape::Disc => Some(self.disc_radius_m()),
        }
    }
}
