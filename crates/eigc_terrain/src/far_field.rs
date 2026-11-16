//! Anel de extensão só visual ao redor do recorte.
//!
//! Blocos de malha grossa, do tamanho do recorte, continuam o relevo e a cor da receita além da
//! borda, para o terreno não terminar num quadrado solto no espaço. Além da borda o relevo
//! recebe a queda da curvatura da lua, que arredonda o horizonte e esconde a borda externa do
//! anel de quem está perto do chão.
//!
//! O anel não tem LOD nem é lido por `follow_ground`: a câmera fica presa dentro do recorte, então
//! ele só existe para ser visto de longe. Como usa a mesma função de altura e de cor da receita,
//! qualquer mudança na receita aparece nele também.

use crate::chunk_mesh::{ChunkRegion, build_chunk_mesh};
use crate::height::{ColorSource, HeightFn, HeightSource};
use bevy::light::NotShadowCaster;
use bevy::pbr::Material;
use bevy::prelude::{
    Assets, Commands, Component, Handle, Mesh, Mesh3d, MeshMaterial3d, Transform, Vec2,
};
use bevy::state::state_scoped::DespawnOnExit;
use eigc_moons::AppState;

/// Configuração do anel de extensão de uma receita.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarFieldConfig {
    /// Quantos blocos, do tamanho do recorte, o anel avança além da borda em cada direção. `1`
    /// cerca o recorte com 8 blocos (grade 3x3 sem o centro).
    pub rings: u32,
    /// Quadrados por lado de cada bloco.
    pub quads_per_tile: u32,
    /// Raio da lua, em metros, usado na queda da curvatura além da borda.
    pub moon_radius_m: f32,
    /// Profundidade do skirt de cada bloco, em fração do espaçamento da malha do bloco. O skirt da
    /// borda interna cobre a diferença entre a malha grossa do bloco e a dos chunks vizinhos.
    pub skirt_depth_factor: f32,
}

/// Marca um bloco do anel de extensão e guarda sua posição na grade de blocos (o recorte é o
/// bloco `(0, 0)`, que não existe no anel).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarFieldTile {
    /// Posição do bloco no eixo X, em blocos a partir do recorte.
    pub ix: i32,
    /// Posição do bloco no eixo Z, em blocos a partir do recorte.
    pub iz: i32,
}

/// Queda da superfície, em metros, causada pela curvatura da lua num ponto além da borda do
/// recorte. É zero dentro do recorte e cresce com o quadrado da distância até ele
/// (`d² / 2R`, aproximação válida para `d` muito menor que `R`), então emenda na borda sem
/// degrau nem quina.
pub fn curvature_drop(x: f32, z: f32, half_extent_m: f32, moon_radius_m: f32) -> f32 {
    let outside = Vec2::new(
        (x.abs() - half_extent_m).max(0.0),
        (z.abs() - half_extent_m).max(0.0),
    );
    outside.length_squared() / (2.0 * moon_radius_m)
}

/// Relevo da receita com a queda da curvatura aplicada além da borda do recorte.
struct CurvedBeyondCutout {
    source: HeightFn,
    half_extent_m: f32,
    moon_radius_m: f32,
}

impl HeightSource for CurvedBeyondCutout {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.source.height_at(x, z) - curvature_drop(x, z, self.half_extent_m, self.moon_radius_m)
    }
}

/// Posição na grade e centro no mundo (`x` -> X, `y` -> Z) de cada bloco do anel, em volta do
/// recorte centrado na origem.
fn far_field_tiles(terrain_size: f32, rings: u32) -> Vec<(FarFieldTile, Vec2)> {
    let rings = rings as i32;
    let mut tiles = Vec::new();
    for iz in -rings..=rings {
        for ix in -rings..=rings {
            if ix == 0 && iz == 0 {
                continue;
            }
            let center = Vec2::new(ix as f32 * terrain_size, iz as f32 * terrain_size);
            tiles.push((FarFieldTile { ix, iz }, center));
        }
    }
    tiles
}

/// Gera a malha de um bloco do anel centrado em `center`.
fn build_far_field_tile_mesh(
    center: Vec2,
    terrain_size: f32,
    config: &FarFieldConfig,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
) -> Mesh {
    let quads = config.quads_per_tile.max(1);
    let skirt_depth = config.skirt_depth_factor * terrain_size / quads as f32;
    let region = ChunkRegion {
        center,
        size: terrain_size,
    };
    build_chunk_mesh(region, quads, skirt_depth, height, color)
}

/// Spawna os blocos do anel de extensão ao redor do recorte, com o mesmo material dos chunks.
///
/// Roda uma vez, junto com os chunks, ao entrar em `AppState::Running`; os blocos saem por
/// `DespawnOnExit`. Não projetam sombra: ficam além do alcance útil das cascatas e só pesariam no
/// passe de sombra.
pub fn spawn_far_field<M: Material>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<M>,
    terrain_size: f32,
    config: &FarFieldConfig,
    height: &HeightFn,
    color: Option<&dyn ColorSource>,
) {
    let curved = CurvedBeyondCutout {
        source: height.clone(),
        half_extent_m: terrain_size * 0.5,
        moon_radius_m: config.moon_radius_m,
    };

    for (tile, center) in far_field_tiles(terrain_size, config.rings) {
        let mesh = build_far_field_tile_mesh(center, terrain_size, config, &curved, color);
        commands.spawn((
            tile,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(center.x, 0.0, center.y),
            NotShadowCaster,
            DespawnOnExit(AppState::Running),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height::arc;
    use bevy::mesh::VertexAttributeValues;

    /// Raio de Io, usado só como escala realista nos testes.
    const RADIUS_M: f32 = 1_821_600.0;

    /// Terreno ondulado só para teste.
    struct Waves;
    impl HeightSource for Waves {
        fn height_at(&self, x: f32, z: f32) -> f32 {
            10.0 * (x * 0.001).sin() + 5.0 * (z * 0.0007).cos()
        }
    }

    /// Testa que a curvatura não mexe no relevo dentro do recorte nem sobre a borda.
    #[test]
    fn curvature_drop_is_zero_inside_and_on_the_cutout_border() {
        assert_eq!(curvature_drop(0.0, 0.0, 11_000.0, RADIUS_M), 0.0);
        assert_eq!(curvature_drop(-10_999.0, 10_999.0, 11_000.0, RADIUS_M), 0.0);
        assert_eq!(curvature_drop(11_000.0, -4_000.0, 11_000.0, RADIUS_M), 0.0);
    }

    /// Testa que a queda segue `d² / 2R` medida a partir da borda: ~33 m a 11 km além dela em Io.
    #[test]
    fn curvature_drop_grows_with_the_square_of_the_distance_past_the_border() {
        let drop = curvature_drop(22_000.0, 0.0, 11_000.0, RADIUS_M);
        assert!((drop - 33.21).abs() < 0.05, "queda: {drop}");

        let corner = curvature_drop(-14_000.0, 15_000.0, 11_000.0, RADIUS_M);
        let expected = (3_000.0_f32.powi(2) + 4_000.0_f32.powi(2)) / (2.0 * RADIUS_M);
        assert!((corner - expected).abs() < 1e-3, "quina: {corner}");
    }

    /// Testa que o anel cerca o recorte sem cobri-lo e sem repetir blocos.
    #[test]
    fn far_field_tiles_surround_the_cutout_without_covering_it() {
        let one_ring = far_field_tiles(22_000.0, 1);
        assert_eq!(one_ring.len(), 8);
        assert!(one_ring.iter().all(|(_, center)| *center != Vec2::ZERO));

        let two_rings = far_field_tiles(22_000.0, 2);
        assert_eq!(two_rings.len(), 24);
        for (index, (tile, center)) in two_rings.iter().enumerate() {
            assert!(tile.ix.abs() <= 2 && tile.iz.abs() <= 2);
            assert_eq!(*center, Vec2::new(tile.ix as f32, tile.iz as f32) * 22_000.0);
            assert!(two_rings[index + 1..].iter().all(|(other, _)| other != tile));
        }
    }

    /// Testa que a borda interna de um bloco tem a mesma altura que a receita na borda do recorte,
    /// para emendar com os chunks.
    #[test]
    fn far_field_tile_matches_the_recipe_height_on_the_cutout_border() {
        let size = 22_000.0;
        let config = FarFieldConfig {
            rings: 1,
            quads_per_tile: 16,
            moon_radius_m: RADIUS_M,
            skirt_depth_factor: 0.0,
        };
        let curved = CurvedBeyondCutout {
            source: arc(Waves),
            half_extent_m: size * 0.5,
            moon_radius_m: RADIUS_M,
        };
        let center = Vec2::new(size, 0.0);
        let mesh = build_far_field_tile_mesh(center, size, &config, &curved, None);

        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("malha sem posições");
        };
        let border: Vec<&[f32; 3]> = positions
            .iter()
            .filter(|[x, _, _]| (center.x + x - size * 0.5).abs() < 1e-3)
            .collect();
        assert_eq!(border.len(), 17);
        for [x, y, z] in border {
            let expected = Waves.height_at(center.x + x, center.y + z);
            assert!((y - expected).abs() < 1e-3, "borda em z={z}: {y} vs {expected}");
        }

        let [x, y, z] = positions[positions.len() - 1];
        let far_expected = Waves.height_at(center.x + x, center.y + z)
            - curvature_drop(center.x + x, center.y + z, size * 0.5, RADIUS_M);
        assert!((y - far_expected).abs() < 1e-3);
        assert!(far_expected < Waves.height_at(center.x + x, center.y + z) - 100.0);
    }
}
