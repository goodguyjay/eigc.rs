//! Crateras de impacto: tigela escavada, borda elevada e manto de ejecta que cai com o cubo da
//! distância. As maiores têm piso plano e pico central. A idade entra como `freshness`: crateras
//! antigas ficam mais rasas e de borda mais baixa (relaxamento do gelo e erosão por impactos
//! menores).
//!
//! Cada cratera soma um desvio de altura à fonte, relativo à superfície de antes do impacto.
//! Crateras que se sobrepõem somam os desvios. Uma grade espacial limita cada consulta às
//! crateras que alcançam o ponto, porque um recorte cheio de crateras tem centenas delas.

use super::HeightSource;
use super::hash::hash01;
use bevy::prelude::Vec2;
use eigc_common::math::smoothstep;

/// Alcance do manto de ejecta, em raios da borda. Além dele a cratera não mexe na altura.
pub const EJECTA_REACH_RADII: f32 = 2.5;

/// Medidas de uma cratera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CraterSpec {
    /// Centro no plano XZ, em metros.
    pub center: Vec2,
    /// Raio da borda, em metros.
    pub radius_m: f32,
    /// Profundidade do piso abaixo da superfície anterior ao impacto, em metros.
    pub depth_m: f32,
    /// Altura da borda acima da superfície anterior ao impacto, em metros.
    pub rim_height_m: f32,
    /// Fração do raio ocupada pelo piso plano (zero numa tigela simples).
    pub floor_fraction: f32,
    /// Altura do pico central acima do piso, em metros (zero sem pico).
    pub central_peak_height_m: f32,
    /// Raio do pico central, em fração do raio da borda.
    pub central_peak_radius_fraction: f32,
    /// Conservação da cratera, de 0 (antiga e degradada) a 1 (recente).
    pub freshness: f32,
}

impl CraterSpec {
    /// Distância do centro além da qual a cratera não mexe na altura, em metros.
    pub fn reach_m(&self) -> f32 {
        self.radius_m * EJECTA_REACH_RADII
    }

    /// Desvio de altura causado pela cratera no ponto, em metros.
    pub fn height_delta_at(&self, x: f32, z: f32) -> f32 {
        crater_profile(self, Vec2::new(x, z).distance(self.center) / self.radius_m)
    }
}

/// Desvio de altura de uma cratera a `r` raios da borda do centro (`r` = distância / raio).
///
/// Dentro da borda: piso plano até `floor_fraction`, e a parede sobe do piso até a borda numa
/// parábola, mais íngreme junto à borda. Fora da borda: o ejecta cai com o cubo da distância e
/// chega a zero em `EJECTA_REACH_RADII`. O perfil é contínuo na borda.
pub fn crater_profile(spec: &CraterSpec, r: f32) -> f32 {
    let r = r.max(0.0);
    if r >= EJECTA_REACH_RADII {
        return 0.0;
    }
    if r >= 1.0 {
        let tail = EJECTA_REACH_RADII.powi(-3);
        return spec.rim_height_m * (r.powi(-3) - tail) / (1.0 - tail);
    }

    let peak = if spec.central_peak_height_m > 0.0 {
        spec.central_peak_height_m
            * (1.0 - smoothstep(0.0, spec.central_peak_radius_fraction, r))
    } else {
        0.0
    };
    let floor = spec.floor_fraction.clamp(0.0, 0.95);
    if r <= floor {
        return -spec.depth_m + peak;
    }
    let t = (r - floor) / (1.0 - floor);
    -spec.depth_m + (spec.depth_m + spec.rim_height_m) * t * t + peak
}

/// Faixas e regras das crateras sorteadas por `generate_crater_specs`.
#[derive(Clone, Copy, Debug)]
pub struct CraterScale {
    /// Menor raio de borda, em metros.
    pub radius_min_m: f32,
    /// Maior raio de borda, em metros.
    pub radius_max_m: f32,
    /// Expoente da distribuição de tamanhos: o número de crateras com raio maior que `R` cai
    /// com `R^-size_exponent`.
    pub size_exponent: f32,
    /// Razão profundidade/diâmetro de uma cratera recente pequena (tigela simples).
    pub depth_ratio_small: f32,
    /// Razão profundidade/diâmetro de uma cratera recente do tamanho máximo.
    pub depth_ratio_large: f32,
    /// Altura da borda em fração da profundidade, numa cratera recente.
    pub rim_to_depth: f32,
    /// Raio a partir do qual a cratera é complexa: piso plano e pico central.
    pub complex_radius_m: f32,
    /// Fração do raio ocupada pelo piso plano numa cratera complexa.
    pub complex_floor_fraction: f32,
    /// Altura do pico central em fração da profundidade, numa cratera complexa.
    pub central_peak_to_depth: f32,
    /// Raio do pico central em fração do raio da borda.
    pub central_peak_radius_fraction: f32,
    /// Profundidade que sobra numa cratera totalmente degradada, em fração da recente.
    pub degraded_depth_fraction: f32,
    /// Altura de borda que sobra numa cratera totalmente degradada, em fração da recente.
    pub degraded_rim_fraction: f32,
}

/// Valores sorteados por cratera: centro (2), raio, conservação e aceitação pela densidade.
const HASH_SLOTS_PER_CRATER: u32 = 5;

/// Raio sorteado pela distribuição em lei de potência truncada entre `min` e `max`, a partir de
/// `u` em [0, 1) (inversa da distribuição acumulada).
fn power_law_radius(min: f32, max: f32, exponent: f32, u: f32) -> f32 {
    let low = min.powf(-exponent);
    let high = max.powf(-exponent);
    (low - u * (low - high)).powf(-1.0 / exponent)
}

/// Monta uma cratera com centro, raio e conservação dados, pelas regras de `scale`.
pub fn crater_from_scale(
    center: Vec2,
    radius_m: f32,
    freshness: f32,
    scale: &CraterScale,
) -> CraterSpec {
    let size = ((radius_m - scale.radius_min_m) / (scale.radius_max_m - scale.radius_min_m))
        .clamp(0.0, 1.0);
    let depth_ratio =
        scale.depth_ratio_small + (scale.depth_ratio_large - scale.depth_ratio_small) * size;
    let fresh_depth = depth_ratio * 2.0 * radius_m;
    let freshness = freshness.clamp(0.0, 1.0);
    let keep = |degraded: f32| degraded + (1.0 - degraded) * freshness;
    let depth_m = fresh_depth * keep(scale.degraded_depth_fraction);
    let complex = radius_m >= scale.complex_radius_m;

    CraterSpec {
        center,
        radius_m,
        depth_m,
        rim_height_m: fresh_depth * scale.rim_to_depth * keep(scale.degraded_rim_fraction),
        floor_fraction: if complex { scale.complex_floor_fraction } else { 0.0 },
        central_peak_height_m: if complex {
            depth_m * scale.central_peak_to_depth
        } else {
            0.0
        },
        central_peak_radius_fraction: scale.central_peak_radius_fraction,
        freshness,
    }
}

/// Sorteia `attempts` crateras deterministicamente a partir de `seed`, com centro no quadrado de
/// meio-lado `half_extent_m` e raio pela lei de potência de `scale`. Cada tentativa é aceita com a
/// probabilidade `density(centro)` (em [0, 1]), o que deixa regiões mais jovens com menos
/// crateras. Nenhuma cratera aceita alcança (com o ejecta) menos de `keep_out_radius_m` da origem.
/// `freshness(centro, sorteio)` decide a conservação a partir de um valor em [0, 1).
pub fn generate_crater_specs(
    seed: u32,
    half_extent_m: f32,
    attempts: usize,
    keep_out_radius_m: f32,
    scale: &CraterScale,
    density: impl Fn(Vec2) -> f32,
    freshness: impl Fn(Vec2, f32) -> f32,
) -> Vec<CraterSpec> {
    let lerp = |min: f32, max: f32, t: f32| min + (max - min) * t;
    (0..attempts as u32)
        .filter_map(|index| {
            let draw = |slot: u32| hash01(seed, index * HASH_SLOTS_PER_CRATER + slot);
            let center = Vec2::new(
                lerp(-half_extent_m, half_extent_m, draw(0)),
                lerp(-half_extent_m, half_extent_m, draw(1)),
            );
            let radius_m = power_law_radius(
                scale.radius_min_m,
                scale.radius_max_m,
                scale.size_exponent,
                draw(2),
            );
            if center.length() - radius_m * EJECTA_REACH_RADII < keep_out_radius_m {
                return None;
            }
            if draw(4) >= density(center).clamp(0.0, 1.0) {
                return None;
            }
            Some(crater_from_scale(
                center,
                radius_m,
                freshness(center, draw(3)),
                scale,
            ))
        })
        .collect()
}

/// Lado de cada célula da grade espacial, em metros.
const GRID_CELL_M: f32 = 1000.0;

/// Grade que guarda, para cada célula, as crateras cujo alcance cobre parte dela.
struct CraterGrid {
    origin: Vec2,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl CraterGrid {
    fn new(craters: &[CraterSpec]) -> Self {
        let Some((min, max)) = craters.iter().fold(None, |bounds: Option<(Vec2, Vec2)>, c| {
            let reach = Vec2::splat(c.reach_m());
            let (lo, hi) = (c.center - reach, c.center + reach);
            Some(bounds.map_or((lo, hi), |(min, max)| (min.min(lo), max.max(hi))))
        }) else {
            return Self {
                origin: Vec2::ZERO,
                columns: 0,
                rows: 0,
                cells: Vec::new(),
            };
        };

        let columns = ((max.x - min.x) / GRID_CELL_M).ceil().max(1.0) as usize;
        let rows = ((max.y - min.y) / GRID_CELL_M).ceil().max(1.0) as usize;
        let mut cells = vec![Vec::new(); columns * rows];
        for (index, crater) in craters.iter().enumerate() {
            let reach = Vec2::splat(crater.reach_m());
            let (first, last) = (
                ((crater.center - reach - min) / GRID_CELL_M).floor(),
                ((crater.center + reach - min) / GRID_CELL_M).floor(),
            );
            for row in (first.y.max(0.0) as usize)..=(last.y as usize).min(rows - 1) {
                for column in (first.x.max(0.0) as usize)..=(last.x as usize).min(columns - 1) {
                    cells[row * columns + column].push(index as u32);
                }
            }
        }
        Self {
            origin: min,
            columns,
            rows,
            cells,
        }
    }

    /// Índices das crateras que podem alcançar o ponto (vazio fora da grade).
    fn candidates(&self, point: Vec2) -> &[u32] {
        let cell = ((point - self.origin) / GRID_CELL_M).floor();
        if cell.x < 0.0 || cell.y < 0.0 {
            return &[];
        }
        let (column, row) = (cell.x as usize, cell.y as usize);
        if column >= self.columns || row >= self.rows {
            return &[];
        }
        &self.cells[row * self.columns + column]
    }
}

/// Soma as crateras a uma fonte de altura.
pub struct CraterField<S: HeightSource> {
    /// Fonte de altura de antes dos impactos.
    pub source: S,
    craters: Vec<CraterSpec>,
    grid: CraterGrid,
}

impl<S: HeightSource> CraterField<S> {
    /// Soma `craters` a `source`.
    pub fn new(source: S, craters: Vec<CraterSpec>) -> Self {
        let grid = CraterGrid::new(&craters);
        Self {
            source,
            craters,
            grid,
        }
    }

    /// Crateras somadas à fonte.
    pub fn craters(&self) -> &[CraterSpec] {
        &self.craters
    }
}

impl<S: HeightSource> HeightSource for CraterField<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let point = Vec2::new(x, z);
        let delta: f32 = self
            .grid
            .candidates(point)
            .iter()
            .map(|&index| self.craters[index as usize].height_delta_at(x, z))
            .sum();
        self.source.height_at(x, z) + delta
    }
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

    fn scale() -> CraterScale {
        CraterScale {
            radius_min_m: 100.0,
            radius_max_m: 3000.0,
            size_exponent: 2.0,
            depth_ratio_small: 0.2,
            depth_ratio_large: 0.1,
            rim_to_depth: 0.25,
            complex_radius_m: 1500.0,
            complex_floor_fraction: 0.4,
            central_peak_to_depth: 0.3,
            central_peak_radius_fraction: 0.15,
            degraded_depth_fraction: 0.35,
            degraded_rim_fraction: 0.2,
        }
    }

    fn crater(radius_m: f32, freshness: f32) -> CraterSpec {
        crater_from_scale(Vec2::ZERO, radius_m, freshness, &scale())
    }

    /// Testa que o perfil é contínuo na borda, chega ao piso no centro de uma tigela e zera no
    /// fim do ejecta.
    #[test]
    fn crater_profile_is_continuous_at_the_rim_and_vanishes_past_the_ejecta() {
        let spec = crater(200.0, 1.0);
        let inside = crater_profile(&spec, 1.0 - 1e-4);
        let outside = crater_profile(&spec, 1.0 + 1e-4);

        assert!((inside - spec.rim_height_m).abs() < 0.05, "dentro {inside}");
        assert!((outside - spec.rim_height_m).abs() < 0.05, "fora {outside}");
        assert_eq!(crater_profile(&spec, 0.0), -spec.depth_m);
        assert_eq!(crater_profile(&spec, EJECTA_REACH_RADII), 0.0);
        assert!(crater_profile(&spec, 0.5 * (1.0 + EJECTA_REACH_RADII)) > 0.0);
    }

    /// Testa que a parede sobe sem voltar do centro até a borda, e o ejecta só desce dali para
    /// fora (cratera simples, sem pico).
    #[test]
    fn crater_profile_rises_to_the_rim_and_falls_beyond_it() {
        let spec = crater(300.0, 1.0);
        let samples: Vec<f32> = (0..=250)
            .map(|i| crater_profile(&spec, i as f32 / 100.0))
            .collect();
        let (wall, ejecta) = samples.split_at(101);
        assert!(wall.windows(2).all(|pair| pair[1] >= pair[0]), "parede");
        assert!(ejecta.windows(2).all(|pair| pair[1] <= pair[0]), "ejecta");
    }

    /// Testa que só as crateras complexas têm piso plano e pico central, e que o pico fica
    /// abaixo da borda.
    #[test]
    fn only_complex_craters_have_a_flat_floor_and_a_central_peak() {
        let simple = crater(800.0, 1.0);
        let complex = crater(2500.0, 1.0);

        assert_eq!(simple.floor_fraction, 0.0);
        assert_eq!(simple.central_peak_height_m, 0.0);
        assert!(complex.floor_fraction > 0.0);
        assert!(crater_profile(&complex, 0.0) > crater_profile(&complex, 0.3));
        assert!(crater_profile(&complex, 0.0) < complex.rim_height_m);
    }

    /// Testa que a cratera degradada é mais rasa e tem borda mais baixa que a recente do mesmo
    /// tamanho.
    #[test]
    fn degraded_craters_are_shallower_with_lower_rims() {
        let fresh = crater(500.0, 1.0);
        let old = crater(500.0, 0.0);

        assert!(old.depth_m < 0.5 * fresh.depth_m);
        assert!(old.rim_height_m < 0.5 * fresh.rim_height_m);
        let diameter = 2.0 * 500.0;
        assert!((0.1 * diameter..=0.2 * diameter).contains(&fresh.depth_m), "{fresh:?}");
    }

    fn generated(seed: u32, density: impl Fn(Vec2) -> f32) -> Vec<CraterSpec> {
        generate_crater_specs(seed, 11_000.0, 2000, 350.0, &scale(), density, |_, u| u)
    }

    /// Testa que o sorteio é determinístico, respeita a faixa de raios e não alcança a clareira.
    #[test]
    fn generated_craters_are_deterministic_in_range_and_clear_of_the_origin() {
        let craters = generated(9, |_| 1.0);
        assert_eq!(craters, generated(9, |_| 1.0));
        assert_ne!(craters, generated(10, |_| 1.0));
        for crater in &craters {
            assert!((100.0..=3000.0).contains(&crater.radius_m), "{crater:?}");
            assert!(crater.center.length() - crater.reach_m() >= 350.0, "{crater:?}");
        }
    }

    /// Testa que a distribuição de tamanhos segue a lei de potência: com expoente 2, há cerca de
    /// quatro vezes mais crateras acima de R do que acima de 2R.
    #[test]
    fn generated_sizes_follow_the_power_law() {
        let craters = generated(3, |_| 1.0);
        let above = |radius: f32| craters.iter().filter(|c| c.radius_m > radius).count() as f32;
        let ratio = above(200.0) / above(400.0);
        assert!((3.0..5.0).contains(&ratio), "razão {ratio}");
    }

    /// Testa que a densidade reduz as crateras onde é baixa (metade x > 0 com densidade 0,1).
    #[test]
    fn density_thins_craters_where_it_is_low() {
        let craters = generated(5, |center| if center.x > 0.0 { 0.1 } else { 1.0 });
        let east = craters.iter().filter(|c| c.center.x > 0.0).count() as f32;
        let west = craters.iter().filter(|c| c.center.x <= 0.0).count() as f32;
        assert!(east < 0.2 * west, "leste {east}, oeste {west}");
    }

    /// Testa que a grade espacial dá o mesmo resultado que somar todas as crateras, inclusive
    /// longe delas (fonte intacta).
    #[test]
    fn crater_field_matches_the_brute_force_sum() {
        let craters = generated(11, |_| 1.0);
        let field = CraterField::new(Flat, craters.clone());
        for i in 0..400 {
            let x = -12_000.0 + (i % 20) as f32 * 1_234.5;
            let z = -12_000.0 + (i / 20) as f32 * 1_187.3;
            let brute: f32 = craters.iter().map(|c| c.height_delta_at(x, z)).sum();
            assert!((field.height_at(x, z) - brute).abs() < 1e-3, "({x}, {z})");
        }
        assert_eq!(field.height_at(1.0e6, 1.0e6), 0.0);
    }
}
