//! Medição do erro geométrico de cada nível de LOD de um chunk.
//!
//! O erro de um nível é o maior desvio vertical (m) entre a superfície triangulada naquele nível
//! e o relevo amostrado na grade do nível 0. Chunks lisos têm erro pequeno mesmo nos níveis
//! grossos, e chunks com lineae ou detalhe fino têm erro grande. É isso que permite ao LOD
//! refinar só onde faz diferença visual.

use crate::height::HeightSource;
use crate::lod::{LOD_LEVEL_COUNT, TerrainLodConfig};
use bevy::prelude::Vec2;

/// Erro medido dos níveis 1 em diante (o nível 0 é a referência e tem erro zero), em metros.
/// A entrada `i` é o erro do nível `i + 1`.
pub(crate) type LevelErrors = [f32; LOD_LEVEL_COUNT - 1];

/// Mede o erro de cada nível grosso de um chunk.
///
/// Amostra o relevo na grade do nível 0 e compara com a superfície que o nível `i` desenharia:
/// vértices a cada `stride` amostras, cada célula dividida na mesma diagonal da malha real. O
/// resultado é forçado a ser não decrescente com o nível, o que mantém crescente o limite de
/// distância de cada nível.
pub(crate) fn measure_level_errors(
    height: &dyn HeightSource,
    center: Vec2,
    chunk_size: f32,
    config: &TerrainLodConfig,
) -> LevelErrors {
    let fine_quads = config.quads_per_chunk[0] as usize;
    let side = fine_quads + 1;
    let step = chunk_size / fine_quads as f32;
    let half = chunk_size * 0.5;

    let mut heights = Vec::with_capacity(side * side);
    for row in 0..side {
        for column in 0..side {
            let x = center.x - half + column as f32 * step;
            let z = center.y - half + row as f32 * step;
            heights.push(height.height_at(x, z));
        }
    }

    let mut errors = [0.0_f32; LOD_LEVEL_COUNT - 1];
    for (index, error) in errors.iter_mut().enumerate() {
        let level_quads = config.quads_per_chunk[index + 1] as usize;
        let stride = fine_quads / level_quads.max(1);
        *error = max_deviation(&heights, side, stride);
    }
    for index in 1..errors.len() {
        errors[index] = errors[index].max(errors[index - 1]);
    }
    errors
}

/// Maior desvio vertical entre as amostras da grade fina e a superfície triangulada a cada
/// `stride` amostras.
fn max_deviation(heights: &[f32], side: usize, stride: usize) -> f32 {
    if stride <= 1 {
        return 0.0;
    }

    let last_cell = ((side - 1) / stride).saturating_sub(1);
    let mut worst = 0.0_f32;
    for row in 0..side {
        for column in 0..side {
            let cell_column = (column / stride).min(last_cell);
            let cell_row = (row / stride).min(last_cell);
            let column0 = cell_column * stride;
            let row0 = cell_row * stride;
            let column1 = column0 + stride;
            let row1 = row0 + stride;

            let fx = (column - column0) as f32 / stride as f32;
            let fz = (row - row0) as f32 / stride as f32;
            let at = |c: usize, r: usize| heights[r * side + c];

            // Mesma divisão da malha real: triângulos (i0, i2, i1) e (i1, i2, i3), com diagonal
            // entre (column1, row0) e (column0, row1).
            let interpolated = if fx + fz <= 1.0 {
                let h0 = at(column0, row0);
                h0 + (at(column1, row0) - h0) * fx + (at(column0, row1) - h0) * fz
            } else {
                let h3 = at(column1, row1);
                h3 + (at(column0, row1) - h3) * (1.0 - fx) + (at(column1, row0) - h3) * (1.0 - fz)
            };
            worst = worst.max((heights[row * side + column] - interpolated).abs());
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;
    impl HeightSource for Flat {
        fn height_at(&self, _x: f32, _z: f32) -> f32 {
            0.0
        }
    }

    /// Plano inclinado: a interpolação linear por triângulos é exata.
    struct Slope;
    impl HeightSource for Slope {
        fn height_at(&self, x: f32, z: f32) -> f32 {
            0.3 * x - 0.2 * z + 7.0
        }
    }

    /// Parábola em X: curvatura constante, erro cresce com o espaçamento.
    struct Parabola;
    impl HeightSource for Parabola {
        fn height_at(&self, x: f32, _z: f32) -> f32 {
            0.001 * x * x
        }
    }

    /// Ruído de alta frequência que só a grade fina enxerga.
    struct Jagged;
    impl HeightSource for Jagged {
        fn height_at(&self, x: f32, z: f32) -> f32 {
            ((x * 0.9).sin() + (z * 1.3).cos()) * 4.0
        }
    }

    fn config() -> TerrainLodConfig {
        TerrainLodConfig {
            chunks_per_side: 4,
            quads_per_chunk: [32, 16, 8, 4],
            max_screen_error_px: 1.5,
            min_error_fraction: 0.05,
            hysteresis_fraction: 0.1,
            skirt_depth_factor: 0.35,
            in_flight_vertex_budget: 10_000,
            max_error_tasks_in_flight: 8,
        }
    }

    fn errors_of(height: &dyn HeightSource) -> LevelErrors {
        measure_level_errors(height, Vec2::new(123.0, -456.0), 344.0, &config())
    }

    /// Relevo plano não tem erro em nenhum nível.
    #[test]
    fn flat_terrain_has_zero_error_at_every_level() {
        assert_eq!(errors_of(&Flat), [0.0; LOD_LEVEL_COUNT - 1]);
    }

    /// Um plano inclinado é reproduzido exatamente por qualquer nível.
    #[test]
    fn sloped_plane_has_no_error_because_triangles_interpolate_it_exactly() {
        for error in errors_of(&Slope) {
            assert!(error < 1e-2, "erro inesperado em plano inclinado: {error}");
        }
    }

    /// Curvatura constante: quanto mais grosso o nível, maior o erro.
    #[test]
    fn curved_terrain_error_grows_with_coarser_levels() {
        let errors = errors_of(&Parabola);
        assert!(errors[0] > 0.0);
        assert!(errors[0] < errors[1] && errors[1] < errors[2], "{errors:?}");
    }

    /// O erro nunca diminui ao engrossar o nível, mesmo em relevo irregular.
    #[test]
    fn errors_never_decrease_with_coarser_levels() {
        let errors = errors_of(&Jagged);
        assert!(errors.windows(2).all(|pair| pair[0] <= pair[1]), "{errors:?}");
    }

    /// Relevo irregular tem erro bem maior que relevo suave no mesmo nível.
    #[test]
    fn rough_terrain_has_more_error_than_smooth_terrain() {
        let rough = errors_of(&Jagged);
        let smooth = errors_of(&Parabola);
        assert!(rough[2] > smooth[2] * 2.0, "{rough:?} vs {smooth:?}");
    }
}
