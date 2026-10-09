//! Matemática pura de LOD do terreno: grade de chunks, distância à câmera, limites por erro em
//! pixels, escolha de nível e planejamento de trocas sob orçamento de vértices em construção.
//!
//! Nada aqui toca ECS além do `derive(Resource)` da configuração. Os sistemas só chamam estas
//! funções e aplicam o resultado.

use crate::lod_error::LevelErrors;
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
    /// Erro máximo tolerado na tela, em pixels. Um nível só é usado se o desvio vertical dele,
    /// projetado na tela, for no máximo isto. Menor significa mais detalhe e mais vértices.
    pub max_screen_error_px: f32,
    /// Piso do erro de cada nível como fração do espaçamento entre vértices dele. Garante que um
    /// chunk liso não fique grosso demais perto da câmera (a cor por vértice também perde
    /// resolução ao engrossar, mesmo quando a altura não muda).
    pub min_error_fraction: f32,
    /// Fração de folga em torno de cada limite, para o nível não oscilar na fronteira.
    pub hysteresis_fraction: f32,
    /// Profundidade do skirt como fração do espaçamento entre vértices do nível.
    pub skirt_depth_factor: f32,
    /// Orçamento de vértices das malhas em construção ao mesmo tempo em segundo plano.
    pub in_flight_vertex_budget: u32,
    /// Máximo de medições de erro de chunk em andamento ao mesmo tempo.
    pub max_error_tasks_in_flight: u32,
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

/// Distâncias em que cada nível grosso passa a ser aceitável. A entrada `i` é a distância a
/// partir da qual o nível `i + 1` atende à tolerância de erro.
pub(crate) type LevelThresholds = [f32; LOD_LEVEL_COUNT - 1];

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

/// Converte o erro medido de um chunk em distâncias-limite por nível.
///
/// Um erro de `e` metros a `d` metros da câmera ocupa `e * screen_scale / d` pixels, então o nível
/// atende à tolerância a partir de `d = e * screen_scale / max_screen_error_px`. O erro usado é o
/// maior entre o medido e o piso `min_error_fraction * espaçamento do nível`.
///
/// Com `errors` igual a `None` (chunk ainda não medido) todos os limites são zero, o que mantém o
/// chunk no nível mais grosso até a medição chegar.
pub(crate) fn level_distance_thresholds(
    errors: Option<&LevelErrors>,
    chunk_size: f32,
    config: &TerrainLodConfig,
    screen_scale: f32,
) -> LevelThresholds {
    let mut thresholds = [0.0_f32; LOD_LEVEL_COUNT - 1];
    let Some(errors) = errors else {
        return thresholds;
    };

    for index in 0..LOD_LEVEL_COUNT - 1 {
        let spacing = chunk_size / config.quads_per_chunk[index + 1] as f32;
        let error = errors[index].max(config.min_error_fraction * spacing);
        thresholds[index] = error * screen_scale / config.max_screen_error_px;
        if index > 0 {
            thresholds[index] = thresholds[index].max(thresholds[index - 1]);
        }
    }
    thresholds
}

/// Escolhe o nível de LOD para uma distância, dado o nível atual do chunk.
///
/// Só engrossa quando a distância passa de `limite * (1 + histerese)` e só afina quando cai
/// abaixo de `limite * (1 - histerese)`. Dentro dessa faixa o nível atual é mantido.
pub(crate) fn select_lod_level(
    distance: f32,
    current_level: u8,
    thresholds: &LevelThresholds,
    hysteresis_fraction: f32,
) -> u8 {
    let mut level = usize::from(current_level).min(LOD_LEVEL_COUNT - 1);

    while level < LOD_LEVEL_COUNT - 1 && distance >= thresholds[level] * (1.0 + hysteresis_fraction)
    {
        level += 1;
    }
    while level > 0 && distance < thresholds[level - 1] * (1.0 - hysteresis_fraction) {
        level -= 1;
    }

    level as u8
}

/// Quantidade de vértices da malha de um chunk, incluindo o anel de skirt.
pub(crate) fn chunk_vertex_count(quads: u32) -> usize {
    let side = quads as usize + 1;
    side * side + 4 * side
}

/// Estado de um chunk visto pelo planejador.
pub(crate) struct ChunkLodInput {
    /// Posição do chunk na grade.
    pub coord: ChunkCoord,
    /// Nível da malha que o chunk mostra agora.
    pub level: u8,
    /// Nível da malha em construção em segundo plano, se houver.
    pub pending: Option<u8>,
    /// Distâncias-limite deste chunk (ver [`level_distance_thresholds`]).
    pub thresholds: LevelThresholds,
}

/// Pedido do planejador: levar o chunk ao nível `target`.
///
/// Quando `target` é igual ao nível atual do chunk e há uma construção pendente, o pedido
/// significa cancelar essa construção.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LodRequest {
    /// Chunk a atualizar.
    pub coord: ChunkCoord,
    /// Nível desejado.
    pub target: u8,
}

/// Decide quais chunks começam a construir uma malha nova neste frame.
///
/// O nível de partida da decisão é o da construção pendente, quando existe. Pedidos de
/// cancelamento saem primeiro e não custam nada; depois vêm refinamentos (nível mais fino) antes
/// de degradações, e dentro de cada grupo o mais próximo do foco primeiro. O custo dos pedidos
/// novos, somado ao das construções já em andamento, não passa de `in_flight_vertex_budget`.
/// Quando nada está em andamento, ao menos um pedido sai, mesmo que sozinho estoure o orçamento.
pub(crate) fn plan_lod_updates(
    chunks: impl IntoIterator<Item = ChunkLodInput>,
    focus: Vec3,
    config: &TerrainLodConfig,
    terrain_size: f32,
) -> Vec<LodRequest> {
    let size = chunk_size(terrain_size, config.chunks_per_side);
    let cost_of = |level: u8| chunk_vertex_count(config.quads_per_chunk[usize::from(level)]) as i64;

    let mut in_flight_cost = 0_i64;
    let mut pending_changes: Vec<PendingChange> = Vec::new();
    for chunk in chunks {
        let old_pending_cost = chunk.pending.map_or(0, cost_of);
        in_flight_cost += old_pending_cost;

        let effective_level = chunk.pending.unwrap_or(chunk.level);
        let center = chunk_center(chunk.coord, terrain_size, config.chunks_per_side);
        let distance = distance_to_chunk(focus, center, size);
        let target = select_lod_level(
            distance,
            effective_level,
            &chunk.thresholds,
            config.hysteresis_fraction,
        );
        if target == effective_level {
            continue;
        }
        pending_changes.push(PendingChange {
            coord: chunk.coord,
            target,
            is_cancel: chunk.pending.is_some() && target == chunk.level,
            is_refinement: target < chunk.level,
            old_pending_cost,
            distance,
        });
    }

    pending_changes.sort_by(|a, b| {
        b.is_cancel
            .cmp(&a.is_cancel)
            .then(b.is_refinement.cmp(&a.is_refinement))
            .then(a.distance.total_cmp(&b.distance))
            .then(a.coord.ix.cmp(&b.coord.ix))
            .then(a.coord.iz.cmp(&b.coord.iz))
    });

    let nothing_in_flight = in_flight_cost == 0;
    let mut remaining = i64::from(config.in_flight_vertex_budget) - in_flight_cost;
    let mut started_any = false;
    let mut plan = Vec::new();
    for change in pending_changes {
        if change.is_cancel {
            remaining += change.old_pending_cost;
            plan.push(LodRequest {
                coord: change.coord,
                target: change.target,
            });
            continue;
        }

        let cost = cost_of(change.target);
        let fits = cost <= remaining + change.old_pending_cost;
        if !fits && !(nothing_in_flight && !started_any) {
            break;
        }
        remaining += change.old_pending_cost - cost;
        started_any = true;
        plan.push(LodRequest {
            coord: change.coord,
            target: change.target,
        });
    }
    plan
}

/// Troca de nível candidata, antes de aplicar o orçamento.
struct PendingChange {
    coord: ChunkCoord,
    target: u8,
    is_cancel: bool,
    is_refinement: bool,
    old_pending_cost: i64,
    distance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{prop_assert, proptest};

    /// Terreno de 1600 m com 4x4 chunks de 400 m, para números fáceis de conferir.
    const TEST_TERRAIN_SIZE: f32 = 1600.0;

    /// Limites fixos usados nos testes de seleção e de planejamento.
    const TEST_THRESHOLDS: LevelThresholds = [500.0, 1000.0, 2000.0];

    fn test_config() -> TerrainLodConfig {
        TerrainLodConfig {
            chunks_per_side: 4,
            quads_per_chunk: [32, 16, 8, 4],
            max_screen_error_px: 2.0,
            min_error_fraction: 0.1,
            hysteresis_fraction: 0.1,
            skirt_depth_factor: 0.35,
            in_flight_vertex_budget: 100_000,
            max_error_tasks_in_flight: 8,
        }
    }

    /// Todos os 16 chunks da grade de teste, parados no nível dado e sem construção pendente.
    fn all_chunks_at(level: u8) -> Vec<ChunkLodInput> {
        (0..4)
            .flat_map(|iz| {
                (0..4).map(move |ix| ChunkLodInput {
                    coord: ChunkCoord { ix, iz },
                    level,
                    pending: None,
                    thresholds: TEST_THRESHOLDS,
                })
            })
            .collect()
    }

    /// Limites tão altos que todo chunk quer o nível 0, para testar só o orçamento.
    fn always_finest_thresholds() -> LevelThresholds {
        [1.0e7, 2.0e7, 3.0e7]
    }

    /// Chunk no nível mais grosso querendo o nível 0, com as distâncias do teste.
    fn coarse_input(coord: ChunkCoord) -> ChunkLodInput {
        ChunkLodInput {
            coord,
            level: COARSEST_LEVEL,
            pending: None,
            thresholds: always_finest_thresholds(),
        }
    }

    /// Chunk sobre o qual o foco está em y = 0 tem distância zero.
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

    /// Chunk não medido mantém todos os limites em zero, ou seja, fica no nível mais grosso.
    #[test]
    fn unmeasured_chunk_has_zero_thresholds_and_stays_coarsest() {
        let config = test_config();
        let thresholds = level_distance_thresholds(None, 400.0, &config, 1000.0);
        assert_eq!(thresholds, [0.0; LOD_LEVEL_COUNT - 1]);
        assert_eq!(select_lod_level(0.0, 0, &thresholds, 0.1), COARSEST_LEVEL);
    }

    /// Limite = erro * escala de tela / tolerância em pixels.
    #[test]
    fn thresholds_follow_error_times_screen_scale_over_tolerance() {
        let mut config = test_config();
        config.min_error_fraction = 0.0;
        let errors = [1.0, 2.0, 4.0];
        let thresholds = level_distance_thresholds(Some(&errors), 400.0, &config, 1000.0);
        // tolerância 2 px, escala 1000: erro 1 m -> 500 m.
        assert_eq!(thresholds, [500.0, 1000.0, 2000.0]);
    }

    /// Dobrar a escala de tela (janela maior ou FOV menor) dobra as distâncias-limite.
    #[test]
    fn doubling_screen_scale_doubles_thresholds() {
        let config = test_config();
        let errors = [1.0, 2.0, 4.0];
        let base = level_distance_thresholds(Some(&errors), 400.0, &config, 1000.0);
        let doubled = level_distance_thresholds(Some(&errors), 400.0, &config, 2000.0);
        for (base, doubled) in base.iter().zip(doubled) {
            assert!((doubled - base * 2.0).abs() < 1e-3);
        }
    }

    /// Erro zero (chunk liso) ainda recebe o piso proporcional ao espaçamento.
    #[test]
    fn zero_measured_error_still_gets_the_spacing_floor() {
        let config = test_config();
        let thresholds = level_distance_thresholds(Some(&[0.0; 3]), 400.0, &config, 1000.0);
        // nível 1: espaçamento 400/16 = 25 m, piso 10% = 2,5 m -> 2,5 * 1000 / 2 = 1250 m.
        assert!((thresholds[0] - 1250.0).abs() < 1e-2);
        assert!(thresholds.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    /// Mais erro medido empurra os limites para longe, ou seja, refina de mais longe.
    #[test]
    fn rougher_chunk_keeps_fine_levels_farther_away() {
        let config = test_config();
        let smooth = level_distance_thresholds(Some(&[0.0; 3]), 400.0, &config, 1000.0);
        let rough = level_distance_thresholds(Some(&[20.0, 40.0, 80.0]), 400.0, &config, 1000.0);
        for (smooth, rough) in smooth.iter().zip(rough) {
            assert!(rough > *smooth);
        }
    }

    /// Partindo de qualquer nível, o nível nunca fica mais fino ao afastar.
    #[test]
    fn level_never_gets_finer_as_distance_grows() {
        for start_level in 0..LOD_LEVEL_COUNT as u8 {
            let mut previous = 0_u8;
            for step in 0..400 {
                let distance = step as f32 * 10.0;
                let level = select_lod_level(distance, start_level, &TEST_THRESHOLDS, 0.1);
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
        assert_eq!(select_lod_level(525.0, 0, &TEST_THRESHOLDS, 0.1), 0);
        assert_eq!(select_lod_level(475.0, 1, &TEST_THRESHOLDS, 0.1), 1);
    }

    /// Fora da faixa de histerese o nível troca.
    #[test]
    fn level_changes_outside_the_hysteresis_band() {
        assert_eq!(select_lod_level(600.0, 0, &TEST_THRESHOLDS, 0.1), 1);
        assert_eq!(select_lod_level(400.0, 1, &TEST_THRESHOLDS, 0.1), 0);
    }

    /// Um salto grande de distância pula vários níveis de uma vez.
    #[test]
    fn level_can_skip_multiple_levels_at_once() {
        assert_eq!(select_lod_level(10_000.0, 0, &TEST_THRESHOLDS, 0.1), COARSEST_LEVEL);
        assert_eq!(select_lod_level(0.0, COARSEST_LEVEL, &TEST_THRESHOLDS, 0.1), 0);
    }

    /// Aplicado o plano, um segundo plano sobre o mesmo estado não pede mais nada.
    #[test]
    fn plan_is_empty_when_every_chunk_is_already_at_its_target_level() {
        let config = test_config();
        let focus = Vec3::ZERO;
        let first = plan_lod_updates(all_chunks_at(COARSEST_LEVEL), focus, &config, TEST_TERRAIN_SIZE);

        let settled: Vec<ChunkLodInput> = all_chunks_at(COARSEST_LEVEL)
            .into_iter()
            .map(|input| {
                let level = first
                    .iter()
                    .find(|request| request.coord == input.coord)
                    .map_or(input.level, |request| request.target);
                ChunkLodInput { level, ..input }
            })
            .collect();
        assert!(plan_lod_updates(settled, focus, &config, TEST_TERRAIN_SIZE).is_empty());
    }

    /// O plano nunca gasta mais vértices que o orçamento (fora a primeira troca).
    #[test]
    fn plan_respects_the_in_flight_vertex_budget() {
        let mut config = test_config();
        config.in_flight_vertex_budget = (chunk_vertex_count(32) * 2) as u32;

        let chunks: Vec<ChunkLodInput> = (0..4)
            .map(|ix| coarse_input(ChunkCoord { ix, iz: 0 }))
            .collect();
        let plan = plan_lod_updates(chunks, Vec3::ZERO, &config, TEST_TERRAIN_SIZE);
        assert_eq!(plan.len(), 2);
    }

    /// Com nada em andamento, uma troca por vez é sempre permitida, mesmo acima do orçamento.
    #[test]
    fn plan_always_allows_one_request_when_nothing_is_in_flight() {
        let mut config = test_config();
        config.in_flight_vertex_budget = 1;

        let chunks: Vec<ChunkLodInput> = (0..4)
            .map(|ix| coarse_input(ChunkCoord { ix, iz: 0 }))
            .collect();
        let plan = plan_lod_updates(chunks, Vec3::ZERO, &config, TEST_TERRAIN_SIZE);
        assert_eq!(plan.len(), 1);
    }

    /// Construções já em andamento gastam o orçamento e bloqueiam pedidos novos.
    #[test]
    fn plan_counts_builds_already_in_flight_against_the_budget() {
        let mut config = test_config();
        config.in_flight_vertex_budget = (chunk_vertex_count(32) * 2) as u32;

        let mut chunks: Vec<ChunkLodInput> = (0..4)
            .map(|ix| coarse_input(ChunkCoord { ix, iz: 1 }))
            .collect();
        for in_flight_ix in 0..2 {
            let mut busy = coarse_input(ChunkCoord { ix: in_flight_ix, iz: 0 });
            busy.pending = Some(0);
            chunks.push(busy);
        }
        let plan = plan_lod_updates(chunks, Vec3::ZERO, &config, TEST_TERRAIN_SIZE);
        assert!(plan.is_empty(), "orçamento já está cheio: {plan:?}");
    }

    /// Se o chunk volta a querer o nível que já mostra, a construção pendente é cancelada.
    #[test]
    fn plan_cancels_a_pending_build_when_the_chunk_wants_its_current_level_again() {
        let config = test_config();
        let coord = ChunkCoord { ix: 3, iz: 3 };
        // Chunk mostra o nível 2, constrói o 0 em segundo plano, mas está longe demais para isso.
        let stale = ChunkLodInput {
            coord,
            level: 2,
            pending: Some(0),
            thresholds: TEST_THRESHOLDS,
        };
        let focus = Vec3::new(-600.0, 0.0, -600.0);
        let plan = plan_lod_updates(vec![stale], focus, &config, TEST_TERRAIN_SIZE);
        assert_eq!(plan, vec![LodRequest { coord, target: 2 }]);
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

        let with_level = |coord, level| ChunkLodInput {
            coord,
            level,
            pending: None,
            thresholds: TEST_THRESHOLDS,
        };
        let chunks = vec![
            with_level(coarsen_me, 0),
            with_level(second_nearest, COARSEST_LEVEL),
            with_level(nearest, COARSEST_LEVEL),
        ];
        let plan = plan_lod_updates(chunks, focus, &config, TEST_TERRAIN_SIZE);

        let order: Vec<ChunkCoord> = plan.iter().map(|request| request.coord).collect();
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
