//! Lineae de Europa: pares de cristas de gelo com um vale entre elas, ao longo de um traçado
//! reto/longo (majoritário) ou em arco (minoritário). O arco é uma aproximação geométrica
//! estilizada e deliberadamente exagerada do mecanismo real de tensão de maré. Não modela a
//! física real, serve só para variar o traçado e dar interesse de navegação.

use super::{ColorSource, HeightSource};
use bevy::prelude::Vec2;
use eigc_common::math::{lerp4, smoothstep};
use noise::{NoiseFn, Perlin};

/// Traçado geométrico de uma linea no plano XZ.
#[derive(Clone, Copy, Debug)]
pub enum LineaPath {
    /// Segmento reto entre dois pontos.
    Straight {
        /// Ponto inicial do segmento.
        start: Vec2,
        /// Ponto final do segmento.
        end: Vec2,
    },
    /// Arco circular estilizado.
    Arc {
        /// Centro do círculo que contém o arco.
        center: Vec2,
        /// Raio do arco, em metros.
        radius: f32,
        /// Ângulo inicial do arco, em radianos.
        start_angle: f32,
        /// Ângulo varrido pelo arco a partir de `start_angle`, em radianos.
        sweep_angle: f32,
    },
}

/// Especificação geométrica e física de uma linea individual.
#[derive(Clone, Copy, Debug)]
pub struct LineaSpec {
    /// Traçado central da linea.
    pub path: LineaPath,
    /// Largura de cada crista, em metros (fisicamente 0.2km–4km, tipicamente ~2000.0).
    pub ridge_width_m: f32,
    /// Altura de cada crista, em metros (fisicamente ~100–300).
    pub ridge_height_m: f32,
    /// Largura do vale entre as duas cristas, em metros (fisicamente ~1500.0).
    pub valley_width_m: f32,
    /// Se true, esta é uma linea "protagonista": mais longa/larga.
    pub is_protagonist: bool,
}

impl LineaSpec {
    /// Razão altura/largura da crista. Deve ser sempre ≤0.53 (inclinação máxima ~28°).
    pub fn slope_ratio(&self) -> f32 {
        self.ridge_height_m / self.ridge_width_m
    }

    /// Distância do centro do vale até o centro de cada crista, em metros.
    fn ridge_center_offset(&self) -> f32 {
        self.valley_width_m * 0.5 + self.ridge_width_m * 0.5
    }

    /// Ponto médio aproximado do traçado da linea, no plano XZ.
    pub fn approx_center(&self) -> Vec2 {
        match self.path {
            LineaPath::Straight { start, end } => (start + end) * 0.5,
            LineaPath::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let mid_angle = start_angle + sweep_angle * 0.5;
                center + Vec2::new(mid_angle.cos(), mid_angle.sin()) * radius
            }
        }
    }
}

/// Diferença angular normalizada para [-PI, PI] entre o ângulo de (x, z) em relação a `center` e
/// `start_angle`. Compartilhada entre `distance_to_path` (testa se o ponto cai dentro do
/// intervalo do arco) e `longitudinal_width_scale` (mede posição ao longo do arco).
fn arc_angle_delta(center: Vec2, start_angle: f32, x: f32, z: f32) -> f32 {
    let offset = Vec2::new(x, z) - center;
    let angle = offset.y.atan2(offset.x);

    let mut delta = angle - start_angle;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

/// Distância perpendicular mínima (sempre não-negativa) de um ponto (x, z) ao traçado de uma
/// linea. Para `Straight`, é a distância ao segmento (projeção travada nas extremidades). Para
/// `Arc`, é a distância radial ao círculo, travada ao intervalo angular do arco.
fn distance_to_path(path: &LineaPath, x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    match *path {
        LineaPath::Straight { start, end } => {
            let segment = end - start;
            let len_sq = segment.length_squared();
            if len_sq <= f32::EPSILON {
                return (p - start).length();
            }
            let t = ((p - start).dot(segment) / len_sq).clamp(0.0, 1.0);
            let closest = start + segment * t;
            (p - closest).length()
        }
        LineaPath::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let offset = p - center;
            let dist_to_center = offset.length();
            let delta = arc_angle_delta(center, start_angle, x, z);

            let (lo, hi) = if sweep_angle >= 0.0 {
                (0.0, sweep_angle)
            } else {
                (sweep_angle, 0.0)
            };

            if delta >= lo && delta <= hi {
                (dist_to_center - radius).abs()
            } else {
                // fora do intervalo angular: distância à extremidade mais próxima do arco
                let end_angle = start_angle + sweep_angle;
                let start_point = center + Vec2::new(start_angle.cos(), start_angle.sin()) * radius;
                let end_point = center + Vec2::new(end_angle.cos(), end_angle.sin()) * radius;
                (p - start_point).length().min((p - end_point).length())
            }
        }
    }
}

/// Fração do comprimento da linea, em cada ponta, onde a largura efetiva da crista/vale encolhe
/// suavemente até 0.
const TIP_TAPER_FRACTION: f32 = 0.15;

/// Fator de encolhimento longitudinal em [0,1], medido ao longo do eixo do traçado (não a
/// distância perpendicular, que continua vindo de `distance_to_path`): 1.0 na porção central,
/// decaindo suavemente até 0.0 exatamente nas extremidades e além delas.
fn longitudinal_width_scale(path: &LineaPath, x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let (length, along) = match *path {
        LineaPath::Straight { start, end } => {
            let segment = end - start;
            let length = segment.length();
            if length <= f32::EPSILON {
                return 0.0;
            }
            let along = (p - start).dot(segment) / length;
            (length, along)
        }
        LineaPath::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let length = sweep_angle.abs() * radius;
            if length <= f32::EPSILON {
                return 0.0;
            }
            let delta = arc_angle_delta(center, start_angle, x, z);
            let signed_delta = if sweep_angle >= 0.0 { delta } else { -delta };
            (length, signed_delta * radius)
        }
    };

    let taper_zone = length * TIP_TAPER_FRACTION;
    let dist_from_start = along;
    let dist_from_end = length - along;

    smoothstep(0.0, taper_zone, dist_from_start).min(smoothstep(0.0, taper_zone, dist_from_end))
}

/// Abaixo deste `width_scale`, a linea é tratada como sem influência nenhuma no ponto; perto o
/// bastante da ponta (ou além dela) pra que rodar a fórmula de seção transversal com uma largura
/// quase zero desses resultados instáveis (divisão por um número muito pequeno).
const WIDTH_SCALE_EPSILON: f32 = 1e-3;

/// Calcula o fator de encolhimento longitudinal em (x, z) e retorna uma cópia de `spec` com
/// `ridge_width_m`/`valley_width_m`/`ridge_height_m` escalados por ele, ou `None` se o ponto está
/// na ponta do traçado ou além dela, caso em que a linea não tem influência nenhuma ali (nem
/// altura, nem contribuição pra cor/proximidade).
fn tapered_spec(spec: &LineaSpec, x: f32, z: f32) -> Option<LineaSpec> {
    if matches!(spec.path, LineaPath::Straight { .. }) {
        return Some(*spec);
    }

    let width_scale = longitudinal_width_scale(&spec.path, x, z);
    if width_scale < WIDTH_SCALE_EPSILON {
        return None;
    }

    let mut shrunk = *spec;
    shrunk.ridge_width_m *= width_scale;
    shrunk.valley_width_m *= width_scale;
    shrunk.ridge_height_m *= width_scale;
    Some(shrunk)
}

/// Fator de posição na feição, em [0,1]: 0.0 no fundo do vale (entre as duas cristas), 1.0 na
/// crista/encosta externa ou além. Usado tanto para o perfil de altura quanto para a cor.
pub fn linea_feature_factor(spec: &LineaSpec, d: f32) -> f32 {
    let d = d.abs();
    let valley_half_width = spec.valley_width_m * 0.5;
    if d <= valley_half_width {
        // dentro do vale: 0.0 no centro, sobe suavemente até a base da crista
        smoothstep(0.0, valley_half_width, d)
    } else {
        // na crista e além: sobe de volta a 1.0 no topo da crista e permanece 1.0 depois dela
        let ridge_outer_edge = spec.ridge_center_offset() + spec.ridge_width_m * 0.5;
        smoothstep(valley_half_width, ridge_outer_edge, d).max(smoothstep(
            0.0,
            valley_half_width,
            d,
        ))
    }
}

/// Contribuição de altura de uma linea, em metros, na distância perpendicular `d` (metros, pode
/// ser negativa) ao seu traçado central. Produz duas cristas separadas por um vale raso.
pub fn linea_cross_section_height(spec: &LineaSpec, d: f32) -> f32 {
    let d = d.abs();
    let ridge_center = spec.ridge_center_offset();
    let ridge_half_width = spec.ridge_width_m * 0.5;

    let t = ((d - ridge_center) / ridge_half_width).clamp(-1.0, 1.0);
    let bump = (t * std::f32::consts::FRAC_PI_2).cos().max(0.0);
    spec.ridge_height_m * bump * bump
}

/// Gera um valor pseudo-aleatório determinístico em [0,1) a partir de uma semente e um índice.
fn hash01(seed: u32, index: u32) -> f32 {
    let perlin = Perlin::new(seed ^ index.wrapping_mul(0x9E37_79B9));
    let raw = perlin.get([0.1234_f64, 0.5678_f64]) as f32;
    (raw + 1.0) * 0.5
}

const RIDGE_WIDTH_MIN_M: f32 = 200.0;
const RIDGE_WIDTH_MAX_M: f32 = 4000.0;
/// Teto de largura para lineae regulares.
const REGULAR_WIDTH_MAX_M: f32 = 2600.0;
const RIDGE_HEIGHT_MIN_M: f32 = 100.0;
const RIDGE_HEIGHT_MAX_M: f32 = 300.0;
const MAX_SLOPE_RATIO: f32 = 0.169;
const VALLEY_WIDTH_M: f32 = 1500.0;
/// Comprimento de um arco regular, em metros, faixa absoluta, não proporcional ao tamanho do
/// recorte.
const LINEA_LENGTH_MIN_M: f32 = 6000.0;
const LINEA_LENGTH_MAX_M: f32 = 14000.0;
const PROTAGONIST_LENGTH_SCALE: f32 = 1.6;
const STRAIGHT_PATH_PROBABILITY: f32 = 0.8;

/// Quantidade de famílias de orientação de linea. Europa real registra múltiplas gerações de
/// fratura em orientações distintas, cruzando umas às outras. Valor inicial arbitrário.
const ORIENTATION_FAMILY_COUNT: usize = 3;

/// Deslocamento angular de cada família em relação a `feature_direction`, em radianos.
const ORIENTATION_FAMILY_OFFSETS_RAD: [f32; ORIENTATION_FAMILY_COUNT] = [0.0, 0.8727, -0.8727];

/// Probabilidade de uma linea sair da família 0 (a mais numerosa/base).
const FAMILY_0_PROBABILITY: f32 = 0.5;

/// Rotaciona `feature_direction` pelo deslocamento angular da família `family` (ver
/// `ORIENTATION_FAMILY_OFFSETS_RAD`).
fn family_base_direction(feature_direction: Vec2, family: usize) -> Vec2 {
    let angle = ORIENTATION_FAMILY_OFFSETS_RAD[family];
    Vec2::new(
        feature_direction.x * angle.cos() - feature_direction.y * angle.sin(),
        feature_direction.x * angle.sin() + feature_direction.y * angle.cos(),
    )
    .normalize()
}

/// Sorteia de qual família de orientação uma linea sai, a partir de um valor uniforme `t` em
/// [0,1). Família 0 tem `FAMILY_0_PROBABILITY` de chance; as demais dividem o resto igualmente.
fn pick_orientation_family(t: f32) -> usize {
    if t < FAMILY_0_PROBABILITY || ORIENTATION_FAMILY_COUNT <= 1 {
        return 0;
    }
    let other_count = ORIENTATION_FAMILY_COUNT - 1;
    let remaining = (t - FAMILY_0_PROBABILITY) / (1.0 - FAMILY_0_PROBABILITY);
    1 + ((remaining * other_count as f32) as usize).min(other_count - 1)
}

/// Multiplicador de `terrain_half_extent_m` usado como meio-comprimento de um traçado reto. O
/// traçado reto não sorteia comprimento, ele se estende bem além do recorte visível nos dois
/// sentidos.
const ETERNAL_STRAIGHT_HALF_LENGTH_FACTOR: f32 = 4.0;

/// Fração máxima segura de `ridge_width_m` em relação ao `length` de um arco. Só se aplica a
/// `LineaPath::Arc`.
const MAX_WIDTH_TO_LENGTH_RATIO: f32 = 0.5;

/// Divide o recorte numa grade quadrada alinhada aos eixos (along, ortho) e retorna os centros de
/// célula (antes de jitter) que ficam fora do raio de exclusão ao redor da origem.
///
/// Retorna `(cell_size, centros)`, onde `centros.len() == count`.
fn stratified_cell_centers(
    terrain_half_extent_m: f32,
    count: usize,
    exclusion_radius_m: f32,
) -> (f32, Vec<(f32, f32)>) {
    let mut grid_size = (count as f32).sqrt().ceil().max(1.0) as usize;

    loop {
        let cell_size = (terrain_half_extent_m * 2.0) / grid_size as f32;
        let mut centers = Vec::with_capacity(count);

        'grid: for row in 0..grid_size {
            for col in 0..grid_size {
                let along = -terrain_half_extent_m + cell_size * (col as f32 + 0.5);
                let ortho = -terrain_half_extent_m + cell_size * (row as f32 + 0.5);

                if (along * along + ortho * ortho).sqrt() >= exclusion_radius_m {
                    centers.push((along, ortho));
                    if centers.len() == count {
                        break 'grid;
                    }
                }
            }
        }

        if centers.len() == count {
            return (cell_size, centers);
        }

        grid_size += 1;
    }
}

/// Ponto da reta infinita (que passa por `point` na direção `dir`, unitária) mais próximo da
/// origem.
fn closest_point_on_line_to_origin(point: Vec2, dir: Vec2) -> Vec2 {
    point + dir * (-point).dot(dir)
}

/// Empurra `center` (perpendicularmente a `dir`, sem mudar a posição ao longo da reta) até que a
/// distância perpendicular da reta infinita que passa por `center` na direção `dir` até a origem
/// seja pelo menos `min_distance_m`. Não faz nada se a reta já está longe o bastante.
fn push_line_away_from_origin(center: Vec2, dir: Vec2, normal: Vec2, min_distance_m: f32) -> Vec2 {
    let closest = closest_point_on_line_to_origin(center, dir);
    let perp_distance = closest.length();

    if perp_distance >= min_distance_m {
        return center;
    }

    // `closest` (como vetor a partir da origem) é perpendicular a `dir`
    let push_dir = if perp_distance > f32::EPSILON {
        closest / perp_distance
    } else {
        normal // reta passa exatamente pela origem: empurra numa direção arbitrária
    };
    center + push_dir * (min_distance_m - perp_distance)
}

/// Gera o conjunto determinístico de lineae do recorte, a partir do seed e da direção de feature
/// calibrada. As duas primeiras specs geradas saem marcadas como protagonistas (mais
/// longas/largas, servem de ponto focal); as demais têm dimensões físicas variadas dentro das
/// faixas reais de Europa.
pub fn generate_linea_specs(
    seed: u32,
    feature_direction: Vec2,
    terrain_half_extent_m: f32,
    count: usize,
    spawn_exclusion_radius_m: f32,
) -> Vec<LineaSpec> {
    // a grade de posicionamento não tem mais uma única `feature_direction` pra se alinhar (cada
    // linea agora sorteia sua própria família de orientação, abaixo). Alinha-se à família 0 (a
    // mais numerosa/base) só por que sim, sem significado físico nenhum.
    let grid_dir = family_base_direction(feature_direction, 0);
    let grid_normal = Vec2::new(-grid_dir.y, grid_dir.x);
    let mut specs = Vec::with_capacity(count);

    // posicionamento em grade estratificada (não puramente aleatório): divide o recorte numa
    // grade quadrada alinhada a `grid_dir` e sorteia cada centro numa célula própria,
    // com jitter limitado a uma fração da célula.
    let (cell_size, cell_centers) =
        stratified_cell_centers(terrain_half_extent_m, count, spawn_exclusion_radius_m);
    const JITTER_FRACTION: f32 = 0.35;

    for i in 0..count {
        let index = i as u32;
        let is_protagonist = i < 2;

        // traçado reto agora é "eterno"
        let style_t = hash01(seed, index * 8 + 7);
        let is_straight = style_t < STRAIGHT_PATH_PROBABILITY;

        // largura sorteada de uma subfaixa distinta pra protagonista vs. regular, garantindo por
        // construção que toda protagonista é mais larga que toda linea regular (não é um
        // multiplicador pós-hoc, que poderia perder pra uma regular sorteada no topo da faixa).
        let width_t = hash01(seed, index * 8 + 1);
        let ridge_width_m = if is_protagonist {
            REGULAR_WIDTH_MAX_M + width_t * (RIDGE_WIDTH_MAX_M - REGULAR_WIDTH_MAX_M)
        } else {
            RIDGE_WIDTH_MIN_M + width_t * (REGULAR_WIDTH_MAX_M - RIDGE_WIDTH_MIN_M)
        };

        // comprimento (e a salvaguarda de largura/comprimento) só existem pro arco: o traçado
        // reto é eterno, não tem "comprimento" que uma largura possa exceder.
        let arc_length = if is_straight {
            None
        } else {
            let length_t = hash01(seed, index * 8 + 5);
            let base_length =
                LINEA_LENGTH_MIN_M + length_t * (LINEA_LENGTH_MAX_M - LINEA_LENGTH_MIN_M);
            Some(if is_protagonist {
                base_length * PROTAGONIST_LENGTH_SCALE
            } else {
                base_length
            })
        };

        let ridge_width_m = match arc_length {
            Some(length) => ridge_width_m.min(length * MAX_WIDTH_TO_LENGTH_RATIO),
            None => ridge_width_m,
        };

        // altura derivada da largura já final (nunca sorteada de forma independente nem escalada
        // depois), garantindo a razão altura/largura máxima e o teto físico por construção.
        let height_t = hash01(seed, index * 8 + 2);
        let max_height_for_ratio = ridge_width_m * MAX_SLOPE_RATIO;
        let height_ceiling = RIDGE_HEIGHT_MAX_M.min(max_height_for_ratio);
        let height_floor = RIDGE_HEIGHT_MIN_M.min(height_ceiling);
        let ridge_height_m = height_floor + height_t * (height_ceiling - height_floor);

        let (cell_along, cell_ortho) = cell_centers[i];
        let jitter_range = cell_size * JITTER_FRACTION;

        let center_t = hash01(seed, index * 8 + 3) * 2.0 - 1.0;
        let along_t = hash01(seed, index * 8 + 4) * 2.0 - 1.0;
        let mut center = grid_dir * (cell_along + along_t * jitter_range)
            + grid_normal * (cell_ortho + center_t * jitter_range);

        // família de orientação (ver ORIENTATION_FAMILY_COUNT/ORIENTATION_FAMILY_OFFSETS_RAD):
        // substitui o jitter único em feature_direction, que deixava toda linea quase paralela
        // (convergência de perspectiva, sem cruzamento real).
        let family_t = hash01(seed, index * 8 + 8);
        let family = pick_orientation_family(family_t);
        let family_dir = family_base_direction(feature_direction, family);

        let angle_jitter = (hash01(seed, index * 8 + 6) - 0.5) * 0.3;
        let dir = Vec2::new(
            family_dir.x * angle_jitter.cos() - family_dir.y * angle_jitter.sin(),
            family_dir.x * angle_jitter.sin() + family_dir.y * angle_jitter.cos(),
        )
        .normalize();

        if is_straight {
            // traçado reto eterno: a âncora (center) pode estar longe da origem e a reta ainda
            // assim passar perto/em cima da clareira de spawn, dependendo de `dir`.
            center = push_line_away_from_origin(center, dir, grid_normal, spawn_exclusion_radius_m);
        } else {
            let dist_from_origin = center.length();
            if dist_from_origin < spawn_exclusion_radius_m && dist_from_origin > f32::EPSILON {
                center *= spawn_exclusion_radius_m / dist_from_origin;
            }
        }

        let path = if is_straight {
            // se estende bem além do recorte visível nos dois sentidos, pra que a ponta real
            // (onde a crista teria que existir sem afunilamento).
            let half_length = terrain_half_extent_m * ETERNAL_STRAIGHT_HALF_LENGTH_FACTOR;
            let half = dir * half_length;
            LineaPath::Straight {
                start: center - half,
                end: center + half,
            }
        } else {
            let length = arc_length.expect("arco sempre tem comprimento finito calculado acima");
            let radius = length * 1.2;
            let arc_center = center - dir.perp() * radius;
            // ancora o MEIO do arco em `center` (não o início), igual ao traçado reto
            let anchor_angle = (center - arc_center).y.atan2((center - arc_center).x);
            let sweep_angle = length / radius;
            let start_angle = anchor_angle - sweep_angle * 0.5;
            LineaPath::Arc {
                center: arc_center,
                radius,
                start_angle,
                sweep_angle,
            }
        };

        specs.push(LineaSpec {
            path,
            ridge_width_m,
            ridge_height_m,
            valley_width_m: VALLEY_WIDTH_M,
            is_protagonist,
        });
    }

    specs
}

/// Fonte de altura que soma a contribuição de todas as lineae de um recorte, em metros, mais um
/// ruído de detalhe fino confinado à crista/encosta (quase nulo longe de qualquer linea).
pub struct LineaField {
    /// Lineae que compõem o campo.
    pub specs: Vec<LineaSpec>,
    /// Gerador de ruído usado para o detalhe fino da encosta.
    pub detail_noise: Perlin,
    /// Amplitude do ruído de detalhe fino, em metros (pequena e arbitrária).
    pub detail_amplitude_m: f32,
    /// Frequência do ruído de detalhe fino.
    pub detail_frequency: f32,
}

impl HeightSource for LineaField {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let mut height = 0.0_f32;
        let mut max_proximity = 0.0_f32;

        // max, não soma: duas lineae que passam perto uma da outra não empilham crista sobre
        // crista fisicamente, a mais alta domina.
        for spec in &self.specs {
            let Some(shrunk) = tapered_spec(spec, x, z) else {
                continue;
            };
            let d = distance_to_path(&spec.path, x, z);
            height = height.max(linea_cross_section_height(&shrunk, d));

            let proximity = 1.0 - linea_feature_factor(&shrunk, d).min(1.0);
            max_proximity = max_proximity.max(proximity);
        }

        if max_proximity > 0.0 {
            let detail = self.detail_noise.get([
                (x * self.detail_frequency) as f64,
                (z * self.detail_frequency) as f64,
            ]) as f32;
            height += detail * self.detail_amplitude_m * max_proximity;
        }

        height
    }
}

/// Fonte de cor que pinta claro (gelo) na crista/encosta e escuro/avermelhado no vale, reusando
/// o mesmo conjunto de specs usado por `LineaField` para a geometria.
pub struct LineaColorField {
    /// Lineae que compõem o campo (mesmas specs de `LineaField`).
    pub specs: Vec<LineaSpec>,
    /// Cor da crista/encosta (gelo claro), formato RGBA linear.
    pub ridge_color: [f32; 4],
    /// Cor do vale (escuro/avermelhado), formato RGBA linear.
    pub valley_color: [f32; 4],
}

impl ColorSource for LineaColorField {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let mut valley_weight = 0.0_f32;

        for spec in &self.specs {
            let Some(shrunk) = tapered_spec(spec, x, z) else {
                continue;
            };
            let d = distance_to_path(&spec.path, x, z);
            let factor = linea_feature_factor(&shrunk, d);
            valley_weight = valley_weight.max(1.0 - factor);
        }

        lerp4(self.ridge_color, self.valley_color, valley_weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight_spec() -> LineaSpec {
        LineaSpec {
            path: LineaPath::Straight {
                start: Vec2::new(-1000.0, 0.0),
                end: Vec2::new(1000.0, 0.0),
            },
            ridge_width_m: 2000.0,
            ridge_height_m: 200.0,
            valley_width_m: 1500.0,
            is_protagonist: false,
        }
    }

    #[test]
    fn overlapping_lineae_do_not_stack_height_additively() {
        // duas lineae idênticas passando pelo mesmo ponto: a altura resultante deve ser a da
        // crista mais alta, nunca a soma das duas (cristas não empilham fisicamente quando se
        // aproximam).
        let spec = straight_spec();
        let field = LineaField {
            specs: vec![spec, spec],
            detail_noise: Perlin::new(0),
            detail_amplitude_m: 0.0,
            detail_frequency: 1.0,
        };

        let single = linea_cross_section_height(&spec, 0.0);
        let doubled = field.height_at(0.0, 0.0);

        assert!(
            (doubled - single).abs() < 1e-3,
            "altura com duas lineae sobrepostas ({doubled}) deveria ser igual à de uma só ({single}), não a soma"
        );
    }

    #[test]
    fn distance_to_straight_path_is_perpendicular_and_clamped_at_ends() {
        assert!((distance_to_path(&straight_spec().path, 0.0, 500.0) - 500.0).abs() < 1e-4);
        // além da extremidade do segmento, a distância passa a ser até o ponto final
        let d = distance_to_path(&straight_spec().path, 2000.0, 0.0);
        assert!((d - 1000.0).abs() < 1e-4);
    }

    fn arc_spec() -> LineaSpec {
        LineaSpec {
            path: LineaPath::Arc {
                center: Vec2::new(0.0, -1000.0),
                radius: 1000.0,
                start_angle: 0.0,
                sweep_angle: std::f32::consts::FRAC_PI_2,
            },
            ridge_width_m: 2000.0,
            ridge_height_m: 200.0,
            valley_width_m: 1500.0,
            is_protagonist: false,
        }
    }

    #[test]
    fn straight_linea_never_tapers_and_matches_untapered_profile_anywhere() {
        // tapered_spec devolve a spec original sem encolher pra LineaPath::Straight
        let spec = straight_spec(); // start=(-1000,0), end=(1000,0)
        let field = LineaField {
            specs: vec![spec],
            detail_noise: Perlin::new(0),
            detail_amplitude_m: 0.0,
            detail_frequency: 1.0,
        };
        let ridge_offset = spec.ridge_center_offset();
        let untapered = linea_cross_section_height(&spec, ridge_offset);

        for x in [-1000.0, -700.0, 0.0, 700.0, 1000.0] {
            let height = field.height_at(x, ridge_offset);
            assert!(
                (height - untapered).abs() < 1e-3,
                "altura em x={x} deveria bater com o perfil sem afunilamento em qualquer ponto: {height} vs {untapered}"
            );
        }
    }

    #[test]
    fn arc_taper_vanishes_at_start_and_end_angles_for_any_radius() {
        let spec = arc_spec();
        let LineaPath::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } = spec.path
        else {
            unreachable!()
        };
        let end_angle = start_angle + sweep_angle;

        for r in [radius, radius * 1.1] {
            let start_point = center + Vec2::new(start_angle.cos(), start_angle.sin()) * r;
            let end_point = center + Vec2::new(end_angle.cos(), end_angle.sin()) * r;

            assert!(
                longitudinal_width_scale(&spec.path, start_point.x, start_point.y).abs() < 1e-3
            );
            assert!(longitudinal_width_scale(&spec.path, end_point.x, end_point.y).abs() < 1e-3);
        }

        let mid_angle = start_angle + sweep_angle * 0.5;
        let mid_point = center + Vec2::new(mid_angle.cos(), mid_angle.sin()) * radius;
        assert!(
            (longitudinal_width_scale(&spec.path, mid_point.x, mid_point.y) - 1.0).abs() < 1e-3
        );
    }

    #[test]
    fn cross_section_produces_two_peaks_separated_by_a_valley() {
        let spec = straight_spec();
        let center = linea_cross_section_height(&spec, 0.0);
        let ridge_offset = spec.ridge_center_offset();
        let peak = linea_cross_section_height(&spec, ridge_offset);
        let far = linea_cross_section_height(&spec, ridge_offset + spec.ridge_width_m * 2.0);

        assert!(
            center < peak,
            "centro do vale deveria ser mais baixo que a crista"
        );
        assert!(
            peak > far,
            "crista deveria ser mais alta que o terreno distante"
        );
        assert!((peak - spec.ridge_height_m).abs() < 1e-3);
    }

    #[test]
    fn feature_factor_is_zero_at_valley_center_and_one_far_away() {
        let spec = straight_spec();
        assert_eq!(linea_feature_factor(&spec, 0.0), 0.0);
        let far = spec.ridge_center_offset() + spec.ridge_width_m * 3.0;
        assert!((linea_feature_factor(&spec, far) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hash01_is_deterministic_and_reasonably_spread() {
        let values: Vec<f32> = (0..64).map(|i| hash01(42, i)).collect();
        assert_eq!(values, (0..64).map(|i| hash01(42, i)).collect::<Vec<_>>());

        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));

        let mut buckets = [0u32; 4];
        for v in &values {
            buckets[((v * 4.0) as usize).min(3)] += 1;
        }
        assert!(
            buckets.iter().all(|&count| count > 0),
            "distribuição de hash01 concentrada demais: {buckets:?}"
        );
    }

    #[test]
    fn generate_linea_specs_keeps_a_minimum_spacing_between_centers() {
        // posicionamento em grade estratificada deve evitar que várias lineae se aglomerem no
        // mesmo ponto
        for seed in [1u32, 204, 999, 42] {
            let specs =
                generate_linea_specs(seed, Vec2::new(0.8, 0.2).normalize(), 3000.0, 8, 350.0);
            let centers: Vec<Vec2> = specs.iter().map(LineaSpec::approx_center).collect();

            for i in 0..centers.len() {
                for j in (i + 1)..centers.len() {
                    let distance = (centers[i] - centers[j]).length();
                    assert!(
                        distance > 500.0,
                        "lineae {i} e {j} (seed {seed}) estão a só {distance}m uma da outra"
                    );
                }
            }
        }
    }

    #[test]
    fn generate_linea_specs_keeps_all_centers_outside_spawn_exclusion_radius() {
        // nenhuma linea deveria nascer ancorada em cima da clareira de spawn.
        let exclusion_radius_m = 350.0;
        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(
                seed,
                Vec2::new(0.8, 0.2).normalize(),
                3000.0,
                8,
                exclusion_radius_m,
            );
            for (i, spec) in specs.iter().enumerate() {
                let distance = spec.approx_center().length();
                assert!(
                    distance >= exclusion_radius_m - 1e-3,
                    "linea {i} (seed {seed}) está a {distance}m da origem, dentro do raio de exclusão de {exclusion_radius_m}m"
                );
            }
        }
    }

    #[test]
    fn family_base_direction_matches_expected_offsets() {
        let feature_direction = Vec2::new(1.0, 0.0);
        for family in 0..ORIENTATION_FAMILY_COUNT {
            let dir = family_base_direction(feature_direction, family);
            let actual_angle = dir.y.atan2(dir.x);
            let expected_angle = ORIENTATION_FAMILY_OFFSETS_RAD[family];
            assert!(
                (actual_angle - expected_angle).abs() < 1e-4,
                "família {family}: ângulo {actual_angle} não bate com o deslocamento esperado {expected_angle}"
            );
        }
    }

    #[test]
    fn pick_orientation_family_respects_family_0_probability_and_splits_rest_evenly() {
        assert_eq!(pick_orientation_family(0.0), 0);
        assert_eq!(pick_orientation_family(FAMILY_0_PROBABILITY - 1e-4), 0);
        assert_eq!(pick_orientation_family(FAMILY_0_PROBABILITY + 1e-4), 1);
        assert_eq!(
            pick_orientation_family(1.0 - 1e-4),
            ORIENTATION_FAMILY_COUNT - 1
        );
    }

    #[test]
    fn generate_linea_specs_straight_lineae_span_multiple_orientation_families() {
        // antes, toda linea reta saía em feature_direction ± 0.15 rad (quase paralelas, sem
        // cruzamento real, só convergência de perspectiva).
        let feature_direction = Vec2::new(0.8, 0.2).normalize();
        let feature_angle = feature_direction.y.atan2(feature_direction.x);

        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(seed, feature_direction, 11000.0, 8, 350.0);

            let mut max_deviation = 0.0_f32;
            for spec in &specs {
                if let LineaPath::Straight { start, end } = spec.path {
                    let dir = (end - start).normalize();
                    let angle = dir.y.atan2(dir.x);
                    let mut deviation = (angle - feature_angle).abs();
                    if deviation > std::f32::consts::PI {
                        deviation = std::f32::consts::TAU - deviation;
                    }
                    max_deviation = max_deviation.max(deviation);
                }
            }

            assert!(
                max_deviation > 0.3, // bem acima do antigo jitter único (±0.15 rad = 0.3 rad de span)
                "seed {seed}: maior desvio angular entre lineae retas foi só {max_deviation} rad, \
                 esperava alguma linea em família com deslocamento maior"
            );
        }
    }

    #[test]
    fn generate_linea_specs_straight_lineae_extend_far_beyond_terrain() {
        // traçado reto é "eterno": nunca deveria sortear um comprimento finito derivado de
        // LINEA_LENGTH_MIN/MAX_M
        let terrain_half_extent_m = 11000.0;
        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(
                seed,
                Vec2::new(0.8, 0.2).normalize(),
                terrain_half_extent_m,
                8,
                350.0,
            );
            for (i, spec) in specs.iter().enumerate() {
                if let LineaPath::Straight { start, end } = spec.path {
                    let length = (end - start).length();
                    assert!(
                        length > terrain_half_extent_m * 4.0,
                        "linea reta {i} (seed {seed}) tem comprimento {length}m, deveria se \
                         estender bem além do recorte (meia-extensão {terrain_half_extent_m}m)"
                    );
                }
            }
        }
    }

    #[test]
    fn generate_linea_specs_keeps_straight_lines_away_from_spawn_exclusion() {
        let exclusion_radius_m = 350.0;
        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(
                seed,
                Vec2::new(0.8, 0.2).normalize(),
                11000.0,
                8,
                exclusion_radius_m,
            );
            for (i, spec) in specs.iter().enumerate() {
                if let LineaPath::Straight { start, end } = spec.path {
                    let dir = (end - start).normalize();
                    let distance = closest_point_on_line_to_origin(start, dir).length();
                    assert!(
                        distance >= exclusion_radius_m - 1e-2,
                        "linea reta {i} (seed {seed}) passa a {distance}m da origem, dentro do \
                         raio de exclusão de {exclusion_radius_m}m"
                    );
                }
            }
        }
    }

    #[test]
    fn push_line_away_from_origin_increases_distance_to_min_and_preserves_along_position() {
        let center = Vec2::new(500.0, 100.0);
        let dir = Vec2::new(1.0, 0.0);
        let normal = Vec2::new(0.0, 1.0);
        let min_distance_m = 350.0;

        let pushed = push_line_away_from_origin(center, dir, normal, min_distance_m);

        let new_distance = closest_point_on_line_to_origin(pushed, dir).length();
        assert!(
            (new_distance - min_distance_m).abs() < 1e-3,
            "distância da reta empurrada até a origem deveria bater exatamente no mínimo: {new_distance}"
        );

        // a posição ao longo da reta (projeção em dir) não deveria mudar, só a perpendicular
        assert!(
            (pushed.dot(dir) - center.dot(dir)).abs() < 1e-3,
            "empurrão não deveria alterar a posição ao longo da reta"
        );
    }

    #[test]
    fn push_line_away_from_origin_is_noop_when_already_far_enough() {
        let center = Vec2::new(0.0, 1000.0);
        let dir = Vec2::new(1.0, 0.0);
        let normal = Vec2::new(0.0, 1.0);

        let pushed = push_line_away_from_origin(center, dir, normal, 350.0);
        assert_eq!(pushed, center);
    }

    #[test]
    fn stratified_cell_centers_grows_grid_when_exclusion_leaves_too_few_cells() {
        // com uma grade mínima (3x3 = 9 células) e count=9, excluir a célula central deixaria só
        // 8.
        let (_, centers) = stratified_cell_centers(3000.0, 9, 350.0);
        assert_eq!(centers.len(), 9);
        for (along, ortho) in centers {
            let distance = (along * along + ortho * ortho).sqrt();
            assert!(
                distance >= 350.0,
                "centro a {distance}m cai dentro do raio de exclusão"
            );
        }
    }

    #[test]
    fn generate_linea_specs_is_deterministic() {
        let a = generate_linea_specs(204, Vec2::new(0.8, 0.2).normalize(), 1500.0, 8, 350.0);
        let b = generate_linea_specs(204, Vec2::new(0.8, 0.2).normalize(), 1500.0, 8, 350.0);

        assert_eq!(a.len(), b.len());
        for (sa, sb) in a.iter().zip(b.iter()) {
            assert_eq!(sa.ridge_width_m, sb.ridge_width_m);
            assert_eq!(sa.ridge_height_m, sb.ridge_height_m);
            assert_eq!(sa.is_protagonist, sb.is_protagonist);
        }
    }

    #[test]
    fn generate_linea_specs_marks_exactly_two_protagonists_as_the_largest() {
        let specs = generate_linea_specs(204, Vec2::new(0.8, 0.2).normalize(), 1500.0, 8, 350.0);
        let protagonist_count = specs.iter().filter(|s| s.is_protagonist).count();
        assert_eq!(protagonist_count, 2);

        let max_regular_width = specs
            .iter()
            .filter(|s| !s.is_protagonist)
            .map(|s| s.ridge_width_m)
            .fold(0.0_f32, f32::max);

        for spec in specs.iter().filter(|s| s.is_protagonist) {
            assert!(spec.ridge_width_m >= max_regular_width);
        }
    }

    #[test]
    fn generate_linea_specs_respects_physical_ranges_and_slope_ratio() {
        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(seed, Vec2::new(1.0, 0.0), 1500.0, 8, 350.0);
            for spec in &specs {
                assert!(
                    spec.ridge_width_m >= RIDGE_WIDTH_MIN_M
                        && spec.ridge_width_m <= RIDGE_WIDTH_MAX_M
                );
                assert!(spec.ridge_height_m > 0.0 && spec.ridge_height_m <= RIDGE_HEIGHT_MAX_M);
                assert!(
                    spec.slope_ratio() <= MAX_SLOPE_RATIO + 1e-4,
                    "razão altura/largura {} excede o máximo físico",
                    spec.slope_ratio()
                );
            }
        }
    }
}
