//! Paterae de Io: depressões vulcânicas de piso plano (lava solidificada) cercadas por uma
//! escarpa íngreme, com contorno irregular. Sem borda elevada, como nas paterae reais, que são
//! caldeiras de colapso e não crateras de impacto.

use super::{ColorSource, HeightSource};
use bevy::prelude::Vec2;
use eigc_common::math::{lerp4, smoothstep};

/// Número de harmônicos que deformam o contorno circular da patera (ordens 2, 3 e 4).
pub const OUTLINE_HARMONICS: usize = 3;

/// Medidas de uma patera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PateraSpec {
    /// Centro no plano XZ, em metros.
    pub center: Vec2,
    /// Raio médio da borda (topo da escarpa), em metros.
    pub radius_m: f32,
    /// Profundidade do piso abaixo da altura média do terreno ao redor da borda, em metros.
    pub depth_m: f32,
    /// Largura horizontal da escarpa, medida da borda para dentro, em metros.
    pub scarp_width_m: f32,
    /// Amplitude relativa ao raio de cada harmônico do contorno (ordem `índice + 2`).
    pub outline_amplitudes: [f32; OUTLINE_HARMONICS],
    /// Fase, em radianos, de cada harmônico do contorno.
    pub outline_phases_rad: [f32; OUTLINE_HARMONICS],
}

impl PateraSpec {
    /// Raio da borda na direção `angle_rad` (ângulo de +x girando para +z), já com a deformação
    /// do contorno.
    pub fn rim_radius_at(&self, angle_rad: f32) -> f32 {
        let deformation: f32 = self
            .outline_amplitudes
            .iter()
            .zip(self.outline_phases_rad)
            .enumerate()
            .map(|(i, (amplitude, phase))| amplitude * ((i as f32 + 2.0) * angle_rad + phase).sin())
            .sum();
        self.radius_m * (1.0 + deformation)
    }

    /// Maior raio de borda possível em qualquer direção, usado como limite rápido.
    pub fn max_rim_radius(&self) -> f32 {
        let total: f32 = self.outline_amplitudes.iter().map(|a| a.abs()).sum();
        self.radius_m * (1.0 + total)
    }

    /// Distância do ponto ao centro e raio da borda na direção do ponto.
    fn distance_and_rim(&self, x: f32, z: f32) -> (f32, f32) {
        let offset = Vec2::new(x, z) - self.center;
        (
            offset.length(),
            self.rim_radius_at(offset.y.atan2(offset.x)),
        )
    }

    /// Peso do piso no ponto: `1` no piso plano, `0` fora da borda, transição suave na escarpa.
    pub fn floor_weight_at(&self, x: f32, z: f32) -> f32 {
        let (distance, rim) = self.distance_and_rim(x, z);
        1.0 - smoothstep(rim - self.scarp_width_m, rim, distance)
    }
}

/// Faixas das medidas sorteadas por `generate_patera_specs`.
#[derive(Clone, Copy, Debug)]
pub struct PateraScale {
    /// Menor raio médio da borda, em metros.
    pub radius_min_m: f32,
    /// Maior raio médio da borda, em metros.
    pub radius_max_m: f32,
    /// Menor profundidade, em metros.
    pub depth_min_m: f32,
    /// Maior profundidade, em metros.
    pub depth_max_m: f32,
    /// Inclinação máxima da escarpa (subida/avanço). Define a largura da escarpa a partir da
    /// profundidade: o perfil em smoothstep chega a 1,5 vez a inclinação média.
    pub max_scarp_slope: f32,
    /// Maior amplitude relativa de cada harmônico do contorno.
    pub outline_amplitude_max: f32,
    /// Margem além da borda, em fração do raio máximo, que também precisa ficar fora da zona de
    /// exclusão e longe das outras paterae (ex.: o halo de cor ao redor).
    pub outer_margin_fraction: f32,
}

/// Valores sorteados por tentativa: centro (2), raio, profundidade, amplitudes e fases.
const HASH_SLOTS_PER_ATTEMPT: u32 = 4 + 2 * OUTLINE_HARMONICS as u32;

/// Tentativas de sorteio de cada patera antes de desistir dela.
const MAX_ATTEMPTS_PER_PATERA: u32 = 64;

/// Fração do meio-lado do recorte onde os centros são sorteados, para a patera ficar quase toda
/// dentro do terreno.
const CENTER_EXTENT_FRACTION: f32 = 0.85;

/// Sorteia até `count` paterae deterministicamente a partir de `seed`, com centro dentro de
/// `half_extent_m` da origem. Nenhuma patera (borda mais `outer_margin_fraction`) chega a menos
/// de `keep_out_radius_m` da origem nem se sobrepõe a outra. Uma patera que não cabe em
/// `MAX_ATTEMPTS_PER_PATERA` tentativas é descartada, então o resultado pode ter menos de
/// `count` itens.
pub fn generate_patera_specs(
    seed: u32,
    half_extent_m: f32,
    count: usize,
    keep_out_radius_m: f32,
    scale: &PateraScale,
) -> Vec<PateraSpec> {
    let mut specs: Vec<PateraSpec> = Vec::with_capacity(count);
    let outer = |spec: &PateraSpec| spec.max_rim_radius() * (1.0 + scale.outer_margin_fraction);

    for patera_index in 0..count as u32 {
        for attempt in 0..MAX_ATTEMPTS_PER_PATERA {
            let base = (patera_index * MAX_ATTEMPTS_PER_PATERA + attempt) * HASH_SLOTS_PER_ATTEMPT;
            let candidate =
                draw_patera_spec(|slot| hash01(seed, base + slot), half_extent_m, scale);

            let clear_of_origin =
                candidate.center.length() - outer(&candidate) >= keep_out_radius_m;
            let clear_of_others = specs.iter().all(|other| {
                candidate.center.distance(other.center) >= outer(&candidate) + outer(other)
            });
            if clear_of_origin && clear_of_others {
                specs.push(candidate);
                break;
            }
        }
    }
    specs
}

/// Monta uma patera a partir de valores sorteados em [0, 1).
fn draw_patera_spec(
    draw: impl Fn(u32) -> f32,
    half_extent_m: f32,
    scale: &PateraScale,
) -> PateraSpec {
    let lerp = |min: f32, max: f32, t: f32| min + (max - min) * t;
    let extent = half_extent_m * CENTER_EXTENT_FRACTION;

    let radius_m = lerp(scale.radius_min_m, scale.radius_max_m, draw(2));
    let depth_m = lerp(scale.depth_min_m, scale.depth_max_m, draw(3));
    let mut outline_amplitudes = [0.0; OUTLINE_HARMONICS];
    let mut outline_phases_rad = [0.0; OUTLINE_HARMONICS];
    for i in 0..OUTLINE_HARMONICS {
        outline_amplitudes[i] = scale.outline_amplitude_max * draw(4 + i as u32);
        outline_phases_rad[i] = std::f32::consts::TAU * draw(4 + (OUTLINE_HARMONICS + i) as u32);
    }

    PateraSpec {
        center: Vec2::new(
            lerp(-extent, extent, draw(0)),
            lerp(-extent, extent, draw(1)),
        ),
        radius_m,
        depth_m,
        // derivada máxima do smoothstep é 1,5 / largura
        scarp_width_m: 1.5 * depth_m / scale.max_scarp_slope,
        outline_amplitudes,
        outline_phases_rad,
    }
}

/// Valor pseudo-aleatório determinístico em [0, 1) a partir de uma semente e um índice
/// (mistura splitmix64).
fn hash01(seed: u32, index: u32) -> f32 {
    let mut z = (u64::from(seed) << 32 | u64::from(index)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1_u64 << 24) as f32
}

/// Escava paterae numa fonte de altura: dentro da borda a altura desce suavemente pela escarpa
/// até um piso plano. O nível do piso de cada patera é a média da fonte ao longo da borda menos
/// a profundidade, calculada uma vez em `new`. Onde duas depressões se cruzam vale a mais funda.
pub struct CarvePaterae<S: HeightSource> {
    /// Fonte de altura escavada.
    pub source: S,
    /// Paterae escavadas.
    pub paterae: Vec<PateraSpec>,
    /// Altura do piso de cada patera, na mesma ordem de `paterae`, em metros.
    pub floor_heights: Vec<f32>,
}

impl<S: HeightSource> CarvePaterae<S> {
    /// Pontos amostrados ao longo da borda para calcular o nível do piso.
    const RIM_SAMPLE_COUNT: usize = 32;

    /// Escava `paterae` em `source`, com o piso de cada uma `depth_m` abaixo da altura média da
    /// fonte ao longo da própria borda.
    pub fn new(source: S, paterae: Vec<PateraSpec>) -> Self {
        let floor_heights = paterae
            .iter()
            .map(|spec| {
                let sum: f32 = (0..Self::RIM_SAMPLE_COUNT)
                    .map(|i| {
                        let angle =
                            i as f32 / Self::RIM_SAMPLE_COUNT as f32 * std::f32::consts::TAU;
                        let rim = spec.center + Vec2::from_angle(angle) * spec.rim_radius_at(angle);
                        source.height_at(rim.x, rim.y)
                    })
                    .sum();
                sum / Self::RIM_SAMPLE_COUNT as f32 - spec.depth_m
            })
            .collect();
        Self {
            source,
            paterae,
            floor_heights,
        }
    }
}

impl<S: HeightSource> HeightSource for CarvePaterae<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let full = self.source.height_at(x, z);
        let point = Vec2::new(x, z);
        self.paterae
            .iter()
            .zip(&self.floor_heights)
            .filter(|(spec, _)| point.distance(spec.center) < spec.max_rim_radius())
            .map(|(spec, &floor)| floor + (full - floor) * (1.0 - spec.floor_weight_at(x, z)))
            .fold(full, f32::min)
    }
}

/// Pinta as paterae por cima de uma fonte de cor: piso escuro de lava silicática, que clareia
/// subindo a escarpa, e um halo de depósito avermelhado em anel ao redor da borda, que some até
/// `halo_width_fraction` do raio além dela.
pub struct PateraColorField<C: ColorSource> {
    /// Cor de fundo fora das paterae.
    pub base: C,
    /// Paterae pintadas (as mesmas escavadas no relevo).
    pub paterae: Vec<PateraSpec>,
    /// Cor (RGBA linear) do piso.
    pub floor_color: [f32; 4],
    /// Cor (RGBA linear) do halo.
    pub halo_color: [f32; 4],
    /// Largura do halo além da borda, em fração do raio da borda.
    pub halo_width_fraction: f32,
}

impl<C: ColorSource> ColorSource for PateraColorField<C> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let point = Vec2::new(x, z);
        let mut color = self.base.color_at(x, z);
        for spec in &self.paterae {
            let reach = spec.max_rim_radius() * (1.0 + self.halo_width_fraction);
            if point.distance(spec.center) >= reach {
                continue;
            }
            let (distance, rim) = spec.distance_and_rim(x, z);
            let halo = 1.0 - smoothstep(rim, rim * (1.0 + self.halo_width_fraction), distance);
            color = lerp4(color, self.halo_color, halo);
            color = lerp4(color, self.floor_color, spec.floor_weight_at(x, z));
        }
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que o hash é determinístico, fica em [0, 1) e espalha os valores pela faixa.
    #[test]
    fn hash01_is_deterministic_and_spread_over_unit_interval() {
        let values: Vec<f32> = (0..1000).map(|i| hash01(7, i)).collect();
        assert_eq!(values, (0..1000).map(|i| hash01(7, i)).collect::<Vec<_>>());
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));

        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "média {mean}");
        assert!(values.iter().any(|&v| v < 0.05) && values.iter().any(|&v| v > 0.95));
        assert_ne!(hash01(7, 0), hash01(8, 0));
    }
}
