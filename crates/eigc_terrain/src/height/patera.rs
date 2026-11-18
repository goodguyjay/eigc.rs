//! Paterae de Io: depressões vulcânicas de piso plano (lava solidificada) cercadas por uma
//! escarpa íngreme, com contorno irregular. Sem borda elevada, como nas paterae reais, que são
//! caldeiras de colapso e não crateras de impacto.
//!
//! A escarpa tem o perfil de uma parede de colapso: a borda quebra em pouco espaço, o paredão
//! desce íngreme pela maior parte da profundidade e um talude de detritos, menos inclinado, faz a
//! transição côncava até o piso.

use super::hash::hash01;
use super::{ColorSource, HeightFn, HeightSource};
use bevy::prelude::Vec2;
use eigc_common::math::{lerp4, smoothstep};

/// Número de harmônicos que deformam o contorno circular da patera (ordens 2, 3 e 4).
pub const OUTLINE_HARMONICS: usize = 3;

/// Fração da largura da escarpa, a partir da borda, em que a inclinação cresce de zero até a do
/// paredão (quebra da borda arredondada, sem quina).
const RIM_ROUNDING_FRACTION: f32 = 0.04;

/// Fração da largura da escarpa, a partir da borda, onde o paredão termina e o talude começa.
const CLIFF_END_FRACTION: f32 = 0.25;

/// Fração da profundidade descida até o pé do paredão; o resto fica para o talude.
const CLIFF_DROP_FRACTION: f32 = 0.6;

/// Maior inclinação do perfil normalizado (fração da profundidade por fração da largura), no
/// paredão. A inclinação máxima em metros é `SCARP_PEAK_NORMALIZED_SLOPE * depth_m /
/// scarp_width_m`. O talude começa com ~0,4 dessa inclinação e se achata até o piso.
pub const SCARP_PEAK_NORMALIZED_SLOPE: f32 =
    CLIFF_DROP_FRACTION / (CLIFF_END_FRACTION - RIM_ROUNDING_FRACTION / 2.0);

/// Fração da profundidade já descida em `t`, a posição na escarpa como fração da largura
/// (0 na borda, 1 no pé do talude). Vale 0 para `t <= 0` e 1 para `t >= 1`, é monotônica e tem
/// derivada contínua, exceto no pé do paredão, onde a inclinação cai de propósito.
pub fn scarp_profile(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let slope = SCARP_PEAK_NORMALIZED_SLOPE;
    if t < RIM_ROUNDING_FRACTION {
        slope * t * t / (2.0 * RIM_ROUNDING_FRACTION)
    } else if t < CLIFF_END_FRACTION {
        slope * (t - RIM_ROUNDING_FRACTION / 2.0)
    } else {
        let u = (t - CLIFF_END_FRACTION) / (1.0 - CLIFF_END_FRACTION);
        CLIFF_DROP_FRACTION + (1.0 - CLIFF_DROP_FRACTION) * (1.0 - (1.0 - u) * (1.0 - u))
    }
}

/// Irregularidade da borda em planta (reentrâncias de colapso e esporões do paredão), em escala
/// menor que a dos harmônicos do contorno. O mesmo valor precisa ir para o relevo
/// (`CarvePaterae`) e para a cor (`PateraColorField`), senão a cor do paredão sai do lugar.
#[derive(Clone)]
pub struct OutlineJitter {
    /// Ruído no plano XZ, em [-1, 1] (valores fora são cortados).
    pub noise: HeightFn,
    /// Deslocamento máximo da borda, em metros (positivo alarga a patera).
    pub amplitude_m: f32,
}

impl OutlineJitter {
    /// Deslocamento da borda no ponto, em metros.
    fn offset_at(&self, x: f32, z: f32) -> f32 {
        self.noise.height_at(x, z).clamp(-1.0, 1.0) * self.amplitude_m
    }
}

/// Deslocamento da borda no ponto, em metros (zero sem irregularidade).
fn jitter_at(jitter: Option<&OutlineJitter>, x: f32, z: f32) -> f32 {
    jitter.map_or(0.0, |jitter| jitter.offset_at(x, z))
}

/// Maior deslocamento possível da borda, em metros.
fn jitter_reach(jitter: Option<&OutlineJitter>) -> f32 {
    jitter.map_or(0.0, |jitter| jitter.amplitude_m.abs())
}

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

    /// Posição do ponto na escarpa, em fração da largura: negativa fora da borda, 0 na borda,
    /// 1 no pé do talude e maior que 1 no piso. `rim_offset_m` desloca a borda (positivo alarga).
    pub fn scarp_fraction_at(&self, x: f32, z: f32, rim_offset_m: f32) -> f32 {
        let (distance, rim) = self.distance_and_rim(x, z);
        (rim + rim_offset_m - distance) / self.scarp_width_m
    }

    /// Peso do piso no ponto (fração da profundidade descida, pelo perfil da escarpa): `1` no
    /// piso plano, `0` fora da borda.
    pub fn floor_weight_at(&self, x: f32, z: f32) -> f32 {
        scarp_profile(self.scarp_fraction_at(x, z, 0.0))
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
    /// Inclinação máxima da escarpa (subida/avanço), a do paredão. Define a largura da escarpa
    /// a partir da profundidade (ver `SCARP_PEAK_NORMALIZED_SLOPE`).
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
        scarp_width_m: SCARP_PEAK_NORMALIZED_SLOPE * depth_m / scale.max_scarp_slope,
        outline_amplitudes,
        outline_phases_rad,
    }
}

/// Escava paterae numa fonte de altura: dentro da borda a altura desce pela escarpa até um piso
/// plano. O nível do piso de cada patera é a média da fonte ao longo da borda menos a
/// profundidade, calculada uma vez em `new`. Onde duas depressões se cruzam vale a mais funda.
pub struct CarvePaterae<S: HeightSource> {
    /// Fonte de altura escavada.
    pub source: S,
    /// Paterae escavadas.
    pub paterae: Vec<PateraSpec>,
    /// Altura do piso de cada patera, na mesma ordem de `paterae`, em metros.
    pub floor_heights: Vec<f32>,
    /// Irregularidade da borda em planta; `None` deixa só o contorno dos harmônicos.
    pub outline_jitter: Option<OutlineJitter>,
    /// Relevo somado ao piso (ex.: cristas de pressão da crosta de lava), em metros. Some
    /// subindo a escarpa junto com o peso do piso.
    pub floor_relief: Option<HeightFn>,
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
            outline_jitter: None,
            floor_relief: None,
        }
    }

    /// Acrescenta a irregularidade da borda em planta.
    pub fn with_outline_jitter(mut self, jitter: OutlineJitter) -> Self {
        self.outline_jitter = Some(jitter);
        self
    }

    /// Acrescenta relevo ao piso.
    pub fn with_floor_relief(mut self, relief: HeightFn) -> Self {
        self.floor_relief = Some(relief);
        self
    }
}

impl<S: HeightSource> HeightSource for CarvePaterae<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let full = self.source.height_at(x, z);
        let point = Vec2::new(x, z);
        let jitter = self.outline_jitter.as_ref();
        let reach = jitter_reach(jitter);
        self.paterae
            .iter()
            .zip(&self.floor_heights)
            .filter(|(spec, _)| point.distance(spec.center) < spec.max_rim_radius() + reach)
            .map(|(spec, &floor)| {
                let weight = scarp_profile(spec.scarp_fraction_at(x, z, jitter_at(jitter, x, z)));
                let relief = self
                    .floor_relief
                    .as_ref()
                    .map_or(0.0, |relief| relief.height_at(x, z));
                let floor = floor + relief;
                floor + (full - floor) * (1.0 - weight)
            })
            .fold(full, f32::min)
    }
}

/// Pinta as paterae por cima de uma fonte de cor: halo de depósito avermelhado em anel ao redor
/// da borda, que some até `halo_width_fraction` do raio além dela; rocha exposta no paredão; e
/// piso escuro de lava silicática, que sobe pelo talude misturado aos detritos.
pub struct PateraColorField<C: ColorSource> {
    /// Cor de fundo fora das paterae.
    pub base: C,
    /// Paterae pintadas (as mesmas escavadas no relevo).
    pub paterae: Vec<PateraSpec>,
    /// Cor (RGBA linear) do piso.
    pub floor_color: [f32; 4],
    /// Cor (RGBA linear) do halo.
    pub halo_color: [f32; 4],
    /// Cor (RGBA linear) da rocha exposta no paredão.
    pub wall_color: [f32; 4],
    /// Irregularidade da borda, a mesma passada a `CarvePaterae`.
    pub outline_jitter: Option<OutlineJitter>,
    /// Largura do halo além da borda, em fração do raio da borda.
    pub halo_width_fraction: f32,
}

impl<C: ColorSource> ColorSource for PateraColorField<C> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let point = Vec2::new(x, z);
        let jitter = self.outline_jitter.as_ref();
        let mut color = self.base.color_at(x, z);
        for spec in &self.paterae {
            let reach =
                spec.max_rim_radius() * (1.0 + self.halo_width_fraction) + jitter_reach(jitter);
            if point.distance(spec.center) >= reach {
                continue;
            }
            let (distance, rim) = spec.distance_and_rim(x, z);
            let halo = 1.0 - smoothstep(rim, rim * (1.0 + self.halo_width_fraction), distance);
            let t = spec.scarp_fraction_at(x, z, jitter_at(jitter, x, z));
            let wall = smoothstep(0.0, 2.0 * RIM_ROUNDING_FRACTION, t);
            let floor = smoothstep(CLIFF_END_FRACTION, 1.0, t);
            color = lerp4(color, self.halo_color, halo);
            color = lerp4(color, self.wall_color, wall);
            color = lerp4(color, self.floor_color, floor);
        }
        color
    }
}
