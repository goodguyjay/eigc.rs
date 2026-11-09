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
    /// Largura de cada crista, em metros (faixa em `LineaScale`).
    pub ridge_width_m: f32,
    /// Altura de cada crista, em metros (regra em `LineaScale`).
    pub ridge_height_m: f32,
    /// Largura do vale entre as duas cristas, em metros (proporcional à largura da crista).
    pub valley_width_m: f32,
    /// Se true, esta é uma linea "protagonista": mais longa/larga.
    pub is_protagonist: bool,
}

impl LineaSpec {
    /// Razão altura/largura da crista. Nunca passa de `LineaScale::max_slope_ratio`.
    pub fn slope_ratio(&self) -> f32 {
        self.ridge_height_m / self.ridge_width_m
    }

    /// Distância do centro do vale até o centro de cada crista, em metros.
    fn ridge_center_offset(&self) -> f32 {
        self.valley_width_m * 0.5 + self.ridge_width_m * 0.5
    }

    /// Distância do traçado central até a borda externa das cristas, em metros. Além dela a
    /// linea não contribui com altura nenhuma.
    fn outer_half_width(&self) -> f32 {
        self.ridge_center_offset() + self.ridge_width_m * 0.5
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
/// altura, nem contribuição pra cor/proximidade). Vale para reta e arco.
fn tapered_spec(spec: &LineaSpec, x: f32, z: f32) -> Option<LineaSpec> {
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

/// Faixas de medida e espalhamento de orientação das lineae geradas por `generate_linea_specs`.
/// Dependem da lua e da escala em que as lineae entram no relevo, por isso vêm de fora (ver
/// `europa_recipe`).
#[derive(Clone, Copy, Debug)]
pub struct LineaScale {
    /// Menor largura de crista, em metros.
    pub ridge_width_min_m: f32,
    /// Maior largura de crista das lineae regulares, em metros. As protagonistas sorteiam entre
    /// este valor e `ridge_width_max_m`, então toda protagonista é mais larga que toda regular.
    pub regular_width_max_m: f32,
    /// Maior largura de crista, em metros (só protagonistas chegam aqui).
    pub ridge_width_max_m: f32,
    /// Menor largura do vale, como fração da largura da crista.
    pub valley_to_ridge_min: f32,
    /// Maior largura do vale, como fração da largura da crista.
    pub valley_to_ridge_max: f32,
    /// Menor comprimento do traçado, em metros.
    pub length_min_m: f32,
    /// Maior comprimento das lineae regulares, em metros. As protagonistas sorteiam entre este
    /// valor e `length_max_m`.
    pub regular_length_max_m: f32,
    /// Maior comprimento do traçado, em metros (só protagonistas chegam aqui).
    pub length_max_m: f32,
    /// Altura da crista, em metros, quando a largura vale `reference_width_m`.
    pub height_at_reference_width_m: f32,
    /// Largura de referência da regra de altura, em metros.
    pub reference_width_m: f32,
    /// Expoente da regra de altura: altura = `height_at_reference_width_m` vezes
    /// (largura / `reference_width_m`) elevado a este expoente.
    pub height_width_exponent: f32,
    /// Menor fração da altura da regra que uma linea pode sortear (a maior é 1,0).
    pub height_fraction_min: f32,
    /// Teto da razão altura/largura da crista. Com o perfil cos², a inclinação máxima da encosta
    /// é π vezes esta razão.
    pub max_slope_ratio: f32,
    /// Deslocamento angular, em radianos, das famílias de orientação 1 e 2 em relação à família 0
    /// (que segue a direção de feature): a família 1 gira `+family_offset_rad` e a 2,
    /// `-family_offset_rad`. Com 0, todas as lineae seguem a direção de feature, a menos do
    /// desvio aleatório de cada uma.
    pub family_offset_rad: f32,
}

impl LineaScale {
    /// Altura máxima da crista para uma largura, em metros: a regra de altura, limitada por
    /// `max_slope_ratio`.
    fn max_height_for_width(&self, ridge_width_m: f32) -> f32 {
        let rule = self.height_at_reference_width_m
            * (ridge_width_m / self.reference_width_m).powf(self.height_width_exponent);
        rule.min(ridge_width_m * self.max_slope_ratio)
    }
}

const STRAIGHT_PATH_PROBABILITY: f32 = 0.8;

/// Quantidade de famílias de orientação de linea. Europa real registra múltiplas gerações de
/// fratura em orientações distintas, cruzando umas às outras. Valor inicial arbitrário.
const ORIENTATION_FAMILY_COUNT: usize = 3;

/// Probabilidade de uma linea sair da família 0 (a mais numerosa/base).
const FAMILY_0_PROBABILITY: f32 = 0.5;

/// Deslocamento angular de cada família em relação a `feature_direction`, em radianos: 0 para a
/// família 0, `+family_offset_rad` para a 1 e `-family_offset_rad` para a 2.
fn family_offsets_rad(family_offset_rad: f32) -> [f32; ORIENTATION_FAMILY_COUNT] {
    [0.0, family_offset_rad, -family_offset_rad]
}

/// Rotaciona `feature_direction` pelo deslocamento angular da família `family` (ver
/// `family_offsets_rad`).
fn family_base_direction(feature_direction: Vec2, family: usize, family_offset_rad: f32) -> Vec2 {
    let angle = family_offsets_rad(family_offset_rad)[family];
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

/// Fração máxima segura de `ridge_width_m` em relação ao comprimento do traçado (reta ou arco).
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

/// Quantos valores de `hash01` cada linea reserva; cada sorteio usa um índice próprio dentro
/// dessa faixa.
const HASH_SLOTS_PER_LINEA: u32 = 16;

/// Fração da célula da grade estratificada usada como deslocamento aleatório do centro.
const JITTER_FRACTION: f32 = 0.35;

/// Tentativas de re-sortear uma linea cujo traçado invade a clareira de spawn, antes de
/// afastá-la por translação (ver `push_path_clear_of_origin`).
const CLEARING_REDRAW_ATTEMPTS: u32 = 8;

/// Passo somado à semente a cada re-sorteio. A tentativa 0 usa a semente original.
const REDRAW_SEED_STEP: u32 = 0x85EB_CA6B;

/// Folga, em metros, além da distância mínima exigida entre o traçado e a origem. Garante que a
/// altura da linea dentro da clareira seja exatamente zero, sem resíduo de arredondamento.
const CLEARING_MARGIN_M: f32 = 1.0;

/// Comprimento do traçado, em metros.
fn path_length(path: &LineaPath) -> f32 {
    match *path {
        LineaPath::Straight { start, end } => (end - start).length(),
        LineaPath::Arc {
            radius,
            sweep_angle,
            ..
        } => sweep_angle.abs() * radius,
    }
}

/// Desloca o traçado inteiro por `offset`.
fn translate_path(path: LineaPath, offset: Vec2) -> LineaPath {
    match path {
        LineaPath::Straight { start, end } => LineaPath::Straight {
            start: start + offset,
            end: end + offset,
        },
        LineaPath::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => LineaPath::Arc {
            center: center + offset,
            radius,
            start_angle,
            sweep_angle,
        },
    }
}

/// Distância mínima entre o traçado de `spec` e a origem para que nenhuma parte da linea alcance
/// o disco de raio `clearing_reach_m`.
fn required_clearance(spec: &LineaSpec, clearing_reach_m: f32) -> f32 {
    clearing_reach_m + spec.outer_half_width() + CLEARING_MARGIN_M
}

/// Afasta `spec` da origem por translação até o traçado ficar a pelo menos `required_clearance`
/// dela. Não faz nada se já está longe o bastante. Desloca na direção do ponto médio do traçado
/// (ou `fallback_dir`, se ele cai na origem) pela distância exigida mais meio comprimento: todo
/// ponto do traçado está a no máximo meio comprimento do ponto médio, então a distância fica
/// garantida para reta e arco.
fn push_path_clear_of_origin(
    spec: LineaSpec,
    clearing_reach_m: f32,
    fallback_dir: Vec2,
) -> LineaSpec {
    let required = required_clearance(&spec, clearing_reach_m);
    if distance_to_path(&spec.path, 0.0, 0.0) >= required {
        return spec;
    }

    let push_dir = spec.approx_center().try_normalize().unwrap_or(fallback_dir);
    let offset = push_dir * (required + path_length(&spec.path) * 0.5);
    LineaSpec {
        path: translate_path(spec.path, offset),
        ..spec
    }
}

/// Sorteia a linea `index` na célula `cell` (along, ortho) da grade estratificada. Todo sorteio
/// sai de `hash01(seed, index * HASH_SLOTS_PER_LINEA + k)`, então a mesma semente e o mesmo
/// índice sempre dão a mesma linea.
fn draw_linea_spec(
    seed: u32,
    index: u32,
    cell: (f32, f32),
    cell_size: f32,
    feature_direction: Vec2,
    scale: &LineaScale,
) -> LineaSpec {
    let draw = |slot: u32| hash01(seed, index * HASH_SLOTS_PER_LINEA + slot);
    let is_protagonist = index < 2;
    let is_straight = draw(7) < STRAIGHT_PATH_PROBABILITY;

    // largura e comprimento sorteados de subfaixas distintas para protagonista e regular,
    // garantindo por construção que toda protagonista é maior que toda regular.
    let (width_min, width_max, length_min, length_max) = if is_protagonist {
        (
            scale.regular_width_max_m,
            scale.ridge_width_max_m,
            scale.regular_length_max_m,
            scale.length_max_m,
        )
    } else {
        (
            scale.ridge_width_min_m,
            scale.regular_width_max_m,
            scale.length_min_m,
            scale.regular_length_max_m,
        )
    };
    let length = length_min + draw(5) * (length_max - length_min);
    let ridge_width_m =
        (width_min + draw(1) * (width_max - width_min)).min(length * MAX_WIDTH_TO_LENGTH_RATIO);

    // altura derivada da largura já final, nunca sorteada de forma independente.
    let height_fraction = scale.height_fraction_min + draw(2) * (1.0 - scale.height_fraction_min);
    let ridge_height_m = scale.max_height_for_width(ridge_width_m) * height_fraction;

    let valley_ratio = scale.valley_to_ridge_min
        + draw(9) * (scale.valley_to_ridge_max - scale.valley_to_ridge_min);
    let valley_width_m = ridge_width_m * valley_ratio;

    // a grade se alinha à família 0 (a mais numerosa), sem significado físico.
    let grid_dir = family_base_direction(feature_direction, 0, scale.family_offset_rad);
    let grid_normal = Vec2::new(-grid_dir.y, grid_dir.x);
    let jitter_range = cell_size * JITTER_FRACTION;
    let (cell_along, cell_ortho) = cell;
    let center = grid_dir * (cell_along + (draw(4) * 2.0 - 1.0) * jitter_range)
        + grid_normal * (cell_ortho + (draw(3) * 2.0 - 1.0) * jitter_range);

    let family_dir = family_base_direction(
        feature_direction,
        pick_orientation_family(draw(8)),
        scale.family_offset_rad,
    );
    let angle_jitter = (draw(6) - 0.5) * 0.3;
    let dir = Vec2::new(
        family_dir.x * angle_jitter.cos() - family_dir.y * angle_jitter.sin(),
        family_dir.x * angle_jitter.sin() + family_dir.y * angle_jitter.cos(),
    )
    .normalize();

    let path = if is_straight {
        let half = dir * (length * 0.5);
        LineaPath::Straight {
            start: center - half,
            end: center + half,
        }
    } else {
        let radius = length * 1.2;
        let arc_center = center - dir.perp() * radius;
        // ancora o MEIO do arco em `center` (não o início), igual ao traçado reto
        let anchor_angle = (center - arc_center).y.atan2((center - arc_center).x);
        let sweep_angle = length / radius;
        LineaPath::Arc {
            center: arc_center,
            radius,
            start_angle: anchor_angle - sweep_angle * 0.5,
            sweep_angle,
        }
    };

    LineaSpec {
        path,
        ridge_width_m,
        ridge_height_m,
        valley_width_m,
        is_protagonist,
    }
}

/// Gera o conjunto determinístico de lineae do recorte, a partir do seed e da direção de feature
/// calibrada. As duas primeiras specs geradas saem marcadas como protagonistas (mais
/// longas/largas, servem de ponto focal); as demais têm medidas dentro das faixas de `scale`.
///
/// Nenhuma parte de nenhuma linea alcança o disco de raio `clearing_reach_m` ao redor da origem
/// (clareira de spawn mais a transição): uma linea cujo traçado invade o disco é re-sorteada e,
/// se continuar invadindo, afastada por translação.
pub fn generate_linea_specs(
    seed: u32,
    feature_direction: Vec2,
    terrain_half_extent_m: f32,
    count: usize,
    clearing_reach_m: f32,
    scale: &LineaScale,
) -> Vec<LineaSpec> {
    let (cell_size, cell_centers) =
        stratified_cell_centers(terrain_half_extent_m, count, clearing_reach_m);
    let grid_dir = family_base_direction(feature_direction, 0, scale.family_offset_rad);
    let grid_normal = Vec2::new(-grid_dir.y, grid_dir.x);

    cell_centers
        .into_iter()
        .enumerate()
        .map(|(i, cell)| {
            let index = i as u32;
            let draw_attempt = |attempt: u32| {
                let attempt_seed = seed.wrapping_add(attempt.wrapping_mul(REDRAW_SEED_STEP));
                draw_linea_spec(
                    attempt_seed,
                    index,
                    cell,
                    cell_size,
                    feature_direction,
                    scale,
                )
            };
            let spec = (0..=CLEARING_REDRAW_ATTEMPTS)
                .map(draw_attempt)
                .find(|spec| {
                    distance_to_path(&spec.path, 0.0, 0.0)
                        >= required_clearance(spec, clearing_reach_m)
                })
                .unwrap_or_else(|| draw_attempt(0));
            push_path_clear_of_origin(spec, clearing_reach_m, grid_normal)
        })
        .collect()
}

/// Fonte de altura das lineae de um recorte, em metros: em cada ponto, a altura da crista mais
/// alta entre as lineae que o alcançam. Não tem ruído próprio; o detalhe fino vem de fora (em
/// Europa, a rugosidade fractal de `europa_recipe`).
pub struct LineaField {
    /// Lineae que compõem o campo.
    pub specs: Vec<LineaSpec>,
}

impl HeightSource for LineaField {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        // max, não soma: duas lineae que passam perto uma da outra não empilham crista sobre
        // crista fisicamente, a mais alta domina.
        self.specs
            .iter()
            .filter_map(|spec| {
                let shrunk = tapered_spec(spec, x, z)?;
                Some(linea_cross_section_height(
                    &shrunk,
                    distance_to_path(&spec.path, x, z),
                ))
            })
            .fold(0.0_f32, f32::max)
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
    use proptest::{prop_assert, proptest};

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
        };
        let ridge_offset = spec.ridge_center_offset();

        let single = linea_cross_section_height(&spec, ridge_offset);
        let doubled = field.height_at(0.0, ridge_offset);

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
    fn finite_straight_linea_has_full_profile_in_the_middle_and_vanishes_at_and_beyond_tips() {
        let spec = straight_spec(); // start=(-1000,0), end=(1000,0)
        let field = LineaField { specs: vec![spec] };
        let ridge_offset = spec.ridge_center_offset();
        let untapered = linea_cross_section_height(&spec, ridge_offset);

        // fora das zonas de afinamento (15% do comprimento em cada ponta) o perfil é o completo
        for x in [-600.0, 0.0, 600.0] {
            let height = field.height_at(x, ridge_offset);
            assert!(
                (height - untapered).abs() < 1e-3,
                "altura em x={x} deveria ser a do perfil completo: {height} vs {untapered}"
            );
        }

        for x in [-1000.0, 1000.0, -1001.0, 1001.0, -1500.0, 2500.0] {
            for z in [0.0, ridge_offset, -ridge_offset] {
                assert_eq!(
                    field.height_at(x, z),
                    0.0,
                    "a linea deveria valer zero na ponta e além dela: ({x}, {z})"
                );
            }
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

    /// Escala de teste: as mesmas ordens de grandeza das lineae de Europa (crista de 100 a 400 m,
    /// traçado de 1,5 a 6 km), sem depender da receita.
    fn test_scale() -> LineaScale {
        LineaScale {
            ridge_width_min_m: 100.0,
            regular_width_max_m: 300.0,
            ridge_width_max_m: 400.0,
            valley_to_ridge_min: 0.5,
            valley_to_ridge_max: 1.0,
            length_min_m: 1500.0,
            regular_length_max_m: 4500.0,
            length_max_m: 6000.0,
            height_at_reference_width_m: 13.2,
            reference_width_m: 227.65,
            height_width_exponent: 0.85,
            height_fraction_min: 0.75,
            max_slope_ratio: 0.065,
            family_offset_rad: 0.8727,
        }
    }

    /// Meia extensão do recorte de Europa, em metros.
    const HALF_EXTENT_M: f32 = 11000.0;

    /// Alcance da clareira de spawn de Europa (platô mais transição), em metros.
    const CLEARING_REACH_M: f32 = 350.0;

    fn europa_like_specs(seed: u32) -> Vec<LineaSpec> {
        generate_linea_specs(
            seed,
            Vec2::new(0.8, 0.2).normalize(),
            HALF_EXTENT_M,
            60,
            CLEARING_REACH_M,
            &test_scale(),
        )
    }

    #[test]
    fn generate_linea_specs_keeps_a_minimum_spacing_between_centers() {
        // posicionamento em grade estratificada deve evitar que várias lineae se aglomerem no
        // mesmo ponto
        for seed in [1u32, 204, 999, 42] {
            let centers: Vec<Vec2> = europa_like_specs(seed)
                .iter()
                .map(LineaSpec::approx_center)
                .collect();

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

    proptest! {
        /// Para qualquer semente, nenhuma parte de nenhuma linea alcança a clareira de spawn: o
        /// traçado fica a pelo menos o alcance da clareira mais a meia largura externa da linea.
        #[test]
        fn no_linea_path_reaches_the_spawn_clearing(seed in 0_u32..u32::MAX) {
            for spec in europa_like_specs(seed) {
                let distance = distance_to_path(&spec.path, 0.0, 0.0);
                let minimum = CLEARING_REACH_M + spec.outer_half_width();
                prop_assert!(
                    distance >= minimum,
                    "traçado a {} m da origem, mínimo {} m",
                    distance,
                    minimum
                );
            }
        }
    }

    #[test]
    fn push_path_clear_of_origin_moves_a_path_crossing_the_origin_out_of_the_clearing() {
        let crossing_arc = LineaSpec {
            path: LineaPath::Arc {
                center: Vec2::new(0.0, -1000.0),
                radius: 1000.0,
                start_angle: 0.5,
                sweep_angle: 1.5,
            },
            ..straight_spec()
        };

        for spec in [straight_spec(), crossing_arc] {
            assert!(distance_to_path(&spec.path, 0.0, 0.0) < required_clearance(&spec, 350.0));

            let pushed = push_path_clear_of_origin(spec, 350.0, Vec2::Y);

            assert!(
                distance_to_path(&pushed.path, 0.0, 0.0) >= required_clearance(&pushed, 350.0),
                "o traçado empurrado ainda alcança a clareira: {:?}",
                pushed.path
            );
            assert_eq!(path_length(&pushed.path), path_length(&spec.path));
        }
    }

    #[test]
    fn push_path_clear_of_origin_is_noop_when_already_far_enough() {
        let far = LineaSpec {
            path: LineaPath::Straight {
                start: Vec2::new(-1000.0, 5000.0),
                end: Vec2::new(1000.0, 5000.0),
            },
            ..straight_spec()
        };

        let pushed = push_path_clear_of_origin(far, 350.0, Vec2::Y);
        assert_eq!(pushed.approx_center(), far.approx_center());
    }

    #[test]
    fn family_base_direction_matches_the_configured_offset() {
        let feature_direction = Vec2::new(1.0, 0.0);
        for family_offset_rad in [0.8727, 0.3, 0.0] {
            for (family, expected_angle) in [0.0, family_offset_rad, -family_offset_rad]
                .into_iter()
                .enumerate()
            {
                let dir = family_base_direction(feature_direction, family, family_offset_rad);
                let actual_angle = dir.y.atan2(dir.x);
                assert!(
                    (actual_angle - expected_angle).abs() < 1e-4,
                    "família {family}: ângulo {actual_angle} não bate com o deslocamento esperado {expected_angle}"
                );
            }
        }
    }

    #[test]
    fn zero_family_offset_keeps_every_straight_linea_along_the_feature_direction() {
        // só sobra o desvio aleatório de cada linea, de ±0,15 rad
        let feature_direction = Vec2::new(0.91, -0.41).normalize();
        let scale = LineaScale {
            family_offset_rad: 0.0,
            ..test_scale()
        };
        for seed in [1u32, 204, 999, 42] {
            let specs = generate_linea_specs(
                seed,
                feature_direction,
                HALF_EXTENT_M,
                60,
                CLEARING_REACH_M,
                &scale,
            );
            for spec in &specs {
                if let LineaPath::Straight { start, end } = spec.path {
                    let deviation = (end - start).normalize().angle_to(feature_direction).abs();
                    assert!(
                        deviation <= 0.15 + 1e-4,
                        "seed {seed}: reta a {deviation} rad da direção de feature"
                    );
                }
            }
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
            let mut max_deviation = 0.0_f32;
            for spec in &europa_like_specs(seed) {
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
    fn generate_linea_specs_keeps_every_path_length_within_the_scale_range() {
        // reta e arco têm comprimento finito sorteado, nenhuma reta atravessa o recorte inteiro
        let scale = test_scale();
        for seed in [1u32, 204, 999, 42] {
            for (i, spec) in europa_like_specs(seed).iter().enumerate() {
                let length = path_length(&spec.path);
                assert!(
                    length >= scale.length_min_m - 1e-2 && length <= scale.length_max_m + 1e-2,
                    "linea {i} (seed {seed}) tem comprimento {length} m, fora de {} a {} m",
                    scale.length_min_m,
                    scale.length_max_m
                );
            }
        }
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
        let a = LineaField {
            specs: europa_like_specs(204),
        };
        let b = LineaField {
            specs: europa_like_specs(204),
        };

        assert_eq!(a.specs.len(), b.specs.len());
        for (sa, sb) in a.specs.iter().zip(b.specs.iter()) {
            assert_eq!(sa.ridge_width_m, sb.ridge_width_m);
            assert_eq!(sa.ridge_height_m, sb.ridge_height_m);
            assert_eq!(sa.valley_width_m, sb.valley_width_m);
            assert_eq!(sa.is_protagonist, sb.is_protagonist);
            assert_eq!(sa.approx_center(), sb.approx_center());
        }
        for i in 0..200 {
            let x = -HALF_EXTENT_M + i as f32 * 109.7;
            let z = HALF_EXTENT_M - i as f32 * 97.3;
            assert_eq!(a.height_at(x, z), b.height_at(x, z));
        }
    }

    #[test]
    fn generate_linea_specs_marks_exactly_two_protagonists_as_the_largest() {
        let specs = europa_like_specs(204);
        let protagonist_count = specs.iter().filter(|s| s.is_protagonist).count();
        assert_eq!(protagonist_count, 2);

        let regulars = specs.iter().filter(|s| !s.is_protagonist);
        let max_regular_width = regulars
            .clone()
            .map(|s| s.ridge_width_m)
            .fold(0.0_f32, f32::max);
        let max_regular_length = regulars
            .map(|s| path_length(&s.path))
            .fold(0.0_f32, f32::max);

        for spec in specs.iter().filter(|s| s.is_protagonist) {
            assert!(spec.ridge_width_m >= max_regular_width);
            assert!(path_length(&spec.path) >= max_regular_length - 1e-2);
        }
    }

    #[test]
    fn generate_linea_specs_respects_scale_ranges_and_slope_ratio() {
        let scale = test_scale();
        for seed in [1u32, 204, 999, 42] {
            for spec in &europa_like_specs(seed) {
                assert!(
                    spec.ridge_width_m >= scale.ridge_width_min_m
                        && spec.ridge_width_m <= scale.ridge_width_max_m
                );
                let ceiling = scale.max_height_for_width(spec.ridge_width_m);
                assert!(
                    spec.ridge_height_m >= ceiling * scale.height_fraction_min - 1e-4
                        && spec.ridge_height_m <= ceiling + 1e-4,
                    "altura {} fora da regra para largura {}",
                    spec.ridge_height_m,
                    spec.ridge_width_m
                );
                let valley_ratio = spec.valley_width_m / spec.ridge_width_m;
                assert!(
                    valley_ratio >= scale.valley_to_ridge_min - 1e-4
                        && valley_ratio <= scale.valley_to_ridge_max + 1e-4
                );
                assert!(
                    spec.slope_ratio() <= scale.max_slope_ratio + 1e-4,
                    "razão altura/largura {} excede o teto",
                    spec.slope_ratio()
                );
            }
        }
    }
}
