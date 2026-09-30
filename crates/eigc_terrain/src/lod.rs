//! Matemática pura de LOD do terreno: grade de chunks, distância à câmera, escolha de nível
//! e planejamento de trocas sob orçamento de vértices por frame.
//!
//! Nada aqui toca ECS além do `derive(Resource)` da configuração. O sistema de `Update` só
//! chama estas funções e aplica o resultado.

use bevy::prelude::{Resource, Vec2, Vec3};

/// Quantidade de níveis de LOD. O nível 0 é o mais fino e `LOD_LEVEL_COUNT - 1` o mais grosso.
pub const LOD_LEVEL_COUNT: usize = 4;

/// Configuração do LOD por chunks de uma lua.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TerrainLodConfig {
    /// Número de chunks por lado da grade (a grade tem `chunks_per_side` x `chunks_per_side`).
    pub chunks_per_side: u32,
    /// Quads por lado do chunk em cada nível. Deve ser decrescente e cada valor deve dividir o
    /// anterior, para que os vértices do nível grosso sejam um subconjunto dos do nível fino.
    pub quads_per_chunk: [u32; LOD_LEVEL_COUNT],
    /// Distâncias (m) em que o chunk passa do nível `i` para o `i + 1`. Devem ser crescentes.
    pub distance_thresholds: [f32; LOD_LEVEL_COUNT - 1],
    /// Fração de folga em torno de cada limite, para o nível não oscilar na fronteira.
    pub hysteresis_fraction: f32,
    /// Profundidade do skirt como fração do espaçamento entre vértices do nível.
    pub skirt_depth_factor: f32,
    /// Orçamento de vértices gerados por frame ao trocar o nível dos chunks.
    pub vertex_budget_per_frame: u32,
}

/// Posição de um chunk na grade do terreno.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    /// Índice no eixo X, de 0 (borda -X) até `chunks_per_side - 1`.
    pub ix: u32,
    /// Índice no eixo Z, de 0 (borda -Z) até `chunks_per_side - 1`.
    pub iz: u32,
}

/// Nível mais grosso, usado no spawn inicial de todos os chunks.
pub(crate) const COARSEST_LEVEL: u8 = (LOD_LEVEL_COUNT - 1) as u8;

/// Lado de um chunk em metros.
pub(crate) fn chunk_size(terrain_size: f32, chunks_per_side: u32) -> f32 {
    terrain_size / chunks_per_side as f32
}

/// Centro do chunk no plano XZ do mundo (`x` -> X, `y` -> Z). O terreno é centrado na origem.
pub(crate) fn chunk_center(coord: ChunkCoord, terrain_size: f32, chunks_per_side: u32) -> Vec2 {
    let size = chunk_size(terrain_size, chunks_per_side);
    let half = terrain_size * 0.5;
    Vec2::new(
        -half + (coord.ix as f32 + 0.5) * size,
        -half + (coord.iz as f32 + 0.5) * size,
    )
}

/// Distância do foco ao chunk, medida até o retângulo do chunk projetado em y = 0.
///
/// A amplitude vertical do terreno é desprezível frente aos 22 km do mapa, então a altura do
/// relevo é ignorada. A altitude do foco entra na distância.
pub(crate) fn distance_to_chunk(focus: Vec3, center: Vec2, chunk_size: f32) -> f32 {
    let half = chunk_size * 0.5;
    let dx = ((focus.x - center.x).abs() - half).max(0.0);
    let dz = ((focus.z - center.y).abs() - half).max(0.0);
    Vec3::new(dx, focus.y, dz).length()
}

/// Escolhe o nível de LOD para uma distância, dado o nível atual do chunk.
///
/// Só engrossa quando a distância passa de `limite * (1 + histerese)` e só afina quando cai
/// abaixo de `limite * (1 - histerese)`. Dentro dessa faixa o nível atual é mantido.
pub(crate) fn select_lod_level(distance: f32, current_level: u8, config: &TerrainLodConfig) -> u8 {
    let hysteresis = config.hysteresis_fraction;
    let mut level = usize::from(current_level).min(LOD_LEVEL_COUNT - 1);

    while level < LOD_LEVEL_COUNT - 1
        && distance >= config.distance_thresholds[level] * (1.0 + hysteresis)
    {
        level += 1;
    }
    while level > 0 && distance < config.distance_thresholds[level - 1] * (1.0 - hysteresis) {
        level -= 1;
    }

    level as u8
}

/// Quantidade de vértices da malha de um chunk, incluindo o anel de skirt.
pub(crate) fn chunk_vertex_count(quads: u32) -> usize {
    let side = quads as usize + 1;
    side * side + 4 * side
}

/// Decide quais chunks trocam de nível neste frame.
///
/// Recebe o nível atual de cada chunk e devolve `(chunk, novo_nível)` para os que precisam
/// mudar, respeitando `vertex_budget_per_frame`. Refinamentos (nível mais fino) vêm antes de
/// degradações, e dentro de cada grupo o mais próximo do foco vem primeiro. Sempre devolve ao
/// menos uma troca quando há alguma pendente, mesmo que ela sozinha estoure o orçamento.
pub(crate) fn plan_lod_updates(
    chunks: impl IntoIterator<Item = (ChunkCoord, u8)>,
    focus: Vec3,
    config: &TerrainLodConfig,
    terrain_size: f32,
) -> Vec<(ChunkCoord, u8)> {
    let size = chunk_size(terrain_size, config.chunks_per_side);

    let mut pending: Vec<PendingChange> = chunks
        .into_iter()
        .filter_map(|(coord, current_level)| {
            let center = chunk_center(coord, terrain_size, config.chunks_per_side);
            let distance = distance_to_chunk(focus, center, size);
            let target_level = select_lod_level(distance, current_level, config);
            (target_level != current_level).then_some(PendingChange {
                coord,
                target_level,
                is_refinement: target_level < current_level,
                distance,
            })
        })
        .collect();

    pending.sort_by(|a, b| {
        b.is_refinement
            .cmp(&a.is_refinement)
            .then(a.distance.total_cmp(&b.distance))
            .then(a.coord.ix.cmp(&b.coord.ix))
            .then(a.coord.iz.cmp(&b.coord.iz))
    });

    let mut spent = 0_usize;
    let budget = config.vertex_budget_per_frame as usize;
    let mut plan = Vec::new();
    for change in pending {
        let cost = chunk_vertex_count(config.quads_per_chunk[usize::from(change.target_level)]);
        if !plan.is_empty() && spent + cost > budget {
            break;
        }
        spent += cost;
        plan.push((change.coord, change.target_level));
    }
    plan
}

/// Troca de nível candidata, antes de aplicar o orçamento.
struct PendingChange {
    coord: ChunkCoord,
    target_level: u8,
    is_refinement: bool,
    distance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{prop_assert, proptest};

    /// Terreno de 1600 m com 4x4 chunks de 400 m, para números fáceis de conferir.
    const TEST_TERRAIN_SIZE: f32 = 1600.0;

    fn test_config() -> TerrainLodConfig {
        TerrainLodConfig {
            chunks_per_side: 4,
            quads_per_chunk: [32, 16, 8, 4],
            distance_thresholds: [500.0, 1000.0, 2000.0],
            hysteresis_fraction: 0.1,
            skirt_depth_factor: 0.35,
            vertex_budget_per_frame: 100_000,
        }
    }

    /// Todos os 16 chunks da grade de teste, todos no nível dado.
    fn all_chunks_at(level: u8) -> Vec<(ChunkCoord, u8)> {
        (0..4)
            .flat_map(|iz| (0..4).map(move |ix| (ChunkCoord { ix, iz }, level)))
            .collect()
    }

    /// Chunk contendo a origem do mundo tem distância zero para um foco em y = 0 sobre ele.
    #[test]
    fn distance_is_zero_when_focus_is_over_the_chunk_at_ground_level() {
        let center = Vec2::new(0.0, 0.0);
        assert_eq!(distance_to_chunk(Vec3::new(50.0, 0.0, -50.0), center, 400.0), 0.0);
    }

    /// A altitude do foco conta na distância mesmo estando sobre o chunk.
    #[test]
    fn distance_includes_focus_altitude() {
        let center = Vec2::new(0.0, 0.0);
        let distance = distance_to_chunk(Vec3::new(0.0, 300.0, 0.0), center, 400.0);
        assert!((distance - 300.0).abs() < 1e-4);
    }

    /// Fora do chunk a distância é medida até a borda, não até o centro.
    #[test]
    fn distance_is_measured_to_chunk_edge_not_center() {
        let center = Vec2::new(0.0, 0.0);
        let distance = distance_to_chunk(Vec3::new(500.0, 0.0, 0.0), center, 400.0);
        assert!((distance - 300.0).abs() < 1e-4);
    }

    /// Os centros dos chunks dos cantos ficam a meio chunk da borda do terreno.
    #[test]
    fn corner_chunk_centers_sit_half_a_chunk_inside_the_terrain() {
        let first = chunk_center(ChunkCoord { ix: 0, iz: 0 }, TEST_TERRAIN_SIZE, 4);
        let last = chunk_center(ChunkCoord { ix: 3, iz: 3 }, TEST_TERRAIN_SIZE, 4);
        assert_eq!(first, Vec2::new(-600.0, -600.0));
        assert_eq!(last, Vec2::new(600.0, 600.0));
    }

    /// Partindo do nível mais grosso, o nível nunca aumenta de granularidade ao afastar.
    #[test]
    fn level_never_gets_finer_as_distance_grows() {
        let config = test_config();
        for start_level in 0..LOD_LEVEL_COUNT as u8 {
            let mut previous = 0_u8;
            for step in 0..400 {
                let distance = step as f32 * 10.0;
                let level = select_lod_level(distance, start_level, &config);
                assert!(
                    level >= previous,
                    "nível {level} < {previous} em {distance} m (partida no nível {start_level})"
                );
                previous = level;
            }
        }
    }

    /// Dentro da faixa de histerese o nível atual é mantido nos dois sentidos.
    #[test]
    fn level_is_kept_inside_the_hysteresis_band() {
        let config = test_config();
        assert_eq!(select_lod_level(525.0, 0, &config), 0);
        assert_eq!(select_lod_level(475.0, 1, &config), 1);
    }

    /// Fora da faixa de histerese o nível troca.
    #[test]
    fn level_changes_outside_the_hysteresis_band() {
        let config = test_config();
        assert_eq!(select_lod_level(600.0, 0, &config), 1);
        assert_eq!(select_lod_level(400.0, 1, &config), 0);
    }

    /// Um salto grande de distância pula vários níveis de uma vez.
    #[test]
    fn level_can_skip_multiple_levels_at_once() {
        let config = test_config();
        assert_eq!(select_lod_level(10_000.0, 0, &config), COARSEST_LEVEL);
        assert_eq!(select_lod_level(0.0, COARSEST_LEVEL, &config), 0);
    }

    /// Sem chunk fora do nível-alvo não há nada a fazer.
    #[test]
    fn plan_is_empty_when_every_chunk_is_already_at_its_target_level() {
        let config = test_config();
        let focus = Vec3::new(0.0, 0.0, 0.0);
        let first = plan_lod_updates(all_chunks_at(COARSEST_LEVEL), focus, &config, TEST_TERRAIN_SIZE);
        let settled: Vec<_> = all_chunks_at(COARSEST_LEVEL)
            .into_iter()
            .map(|(coord, level)| {
                let new_level = first
                    .iter()
                    .find(|(planned, _)| *planned == coord)
                    .map_or(level, |(_, new_level)| *new_level);
                (coord, new_level)
            })
            .collect();
        let second = plan_lod_updates(settled, focus, &config, TEST_TERRAIN_SIZE);
        assert!(second.is_empty());
    }

    /// O plano nunca gasta mais vértices que o orçamento (fora a primeira troca).
    #[test]
    fn plan_respects_the_vertex_budget() {
        let mut config = test_config();
        let level0_cost = chunk_vertex_count(config.quads_per_chunk[0]);
        config.vertex_budget_per_frame = (level0_cost * 2) as u32;
        config.distance_thresholds = [1.0e6, 2.0e6, 3.0e6];

        let plan = plan_lod_updates(
            all_chunks_at(COARSEST_LEVEL),
            Vec3::ZERO,
            &config,
            TEST_TERRAIN_SIZE,
        );
        assert_eq!(plan.len(), 2);
    }

    /// Mesmo com o orçamento menor que um chunk, uma troca por frame é sempre permitida.
    #[test]
    fn plan_always_allows_at_least_one_change() {
        let mut config = test_config();
        config.vertex_budget_per_frame = 1;
        config.distance_thresholds = [1.0e6, 2.0e6, 3.0e6];

        let plan = plan_lod_updates(
            all_chunks_at(COARSEST_LEVEL),
            Vec3::ZERO,
            &config,
            TEST_TERRAIN_SIZE,
        );
        assert_eq!(plan.len(), 1);
    }

    /// Refinamentos vêm antes de degradações e o mais próximo do foco vem primeiro.
    #[test]
    fn plan_puts_refinements_first_and_nearest_chunk_first() {
        let config = test_config();
        // Foco sobre o centro do chunk (0, 0): distância 0 até ele, 200 m até (1, 0) e ~1414 m
        // até (3, 3), que está no nível 0 e portanto engrossa.
        let focus = Vec3::new(-600.0, 0.0, -600.0);
        let nearest = ChunkCoord { ix: 0, iz: 0 };
        let second_nearest = ChunkCoord { ix: 1, iz: 0 };
        let coarsen_me = ChunkCoord { ix: 3, iz: 3 };

        let chunks = vec![
            (coarsen_me, 0),
            (second_nearest, COARSEST_LEVEL),
            (nearest, COARSEST_LEVEL),
        ];
        let plan = plan_lod_updates(chunks, focus, &config, TEST_TERRAIN_SIZE);

        let order: Vec<ChunkCoord> = plan.iter().map(|(coord, _)| *coord).collect();
        assert_eq!(order, vec![nearest, second_nearest, coarsen_me]);
    }

    proptest! {
        /// Para qualquer grade, os chunks cobrem o terreno sem lacuna nem sobreposição.
        #[test]
        fn chunk_grid_tiles_the_terrain_without_gaps(
            chunks_per_side in 1_u32..64,
            terrain_size in 100.0_f32..30_000.0,
        ) {
            let size = chunk_size(terrain_size, chunks_per_side);
            let tolerance = terrain_size * 1e-5 + 1e-3;
            let half = terrain_size * 0.5;

            let first = chunk_center(ChunkCoord { ix: 0, iz: 0 }, terrain_size, chunks_per_side);
            let last_index = chunks_per_side - 1;
            let last = chunk_center(
                ChunkCoord { ix: last_index, iz: last_index },
                terrain_size,
                chunks_per_side,
            );
            prop_assert!((first.x - size * 0.5 + half).abs() < tolerance);
            prop_assert!((last.x + size * 0.5 - half).abs() < tolerance);

            for index in 0..last_index {
                let left = chunk_center(ChunkCoord { ix: index, iz: 0 }, terrain_size, chunks_per_side);
                let right = chunk_center(ChunkCoord { ix: index + 1, iz: 0 }, terrain_size, chunks_per_side);
                prop_assert!(((left.x + size * 0.5) - (right.x - size * 0.5)).abs() < tolerance);
            }
        }
    }
}
