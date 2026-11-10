//! Texturas procedurais usadas na cena da abertura: Júpiter e o brilho radial da UI.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::{Image, Vec3};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{ComputeTaskPool, ParallelSliceMut};
use noise::{Fbm, NoiseFn, Perlin};

use super::smoothstep;

const JUPITER_WIDTH: u32 = 2048;
const JUPITER_HEIGHT: u32 = 1024;
const JUPITER_NOISE_SEED: u32 = 7;

const GLOW_SIZE: u32 = 256;

/// Latitude (graus) e cor de cada faixa de referência, interpoladas com smoothstep entre si.
const BAND_COLORS: [(f32, u32); 16] = [
    (-90.0, 0x8a8072),
    (-62.0, 0xab9c86),
    (-46.0, 0xcbb99a),
    (-36.0, 0xa88565),
    (-27.0, 0xe0cfb0),
    (-19.0, 0xb47b50),
    (-12.0, 0xc48d61),
    (-5.0, 0xead9bd),
    (4.0, 0xe4cfac),
    (10.0, 0xa46a42),
    (16.0, 0xb98057),
    (23.0, 0xe2ceab),
    (31.0, 0xb6916b),
    (41.0, 0xcfbd9c),
    (58.0, 0xa99a84),
    (90.0, 0x887e70),
];

const GREAT_RED_SPOT_LAT_DEG: f32 = -22.0;
const GREAT_RED_SPOT_LON_RAD: f32 = PI;

const WHITE_OVAL_LAT_RAD: f32 = -0.71;
const WHITE_OVAL_COUNT: u32 = 5;

const DARK_SPOT_LAT_RAD: f32 = 0.13;
const DARK_SPOT_COUNT: u32 = 7;

/// Gera a textura equiretangular de Júpiter (2048x1024, RGBA8 sRGB), com faixas, a Grande
/// Mancha Vermelha, ovais brancas e manchas escuras, tudo derivado de ruído Perlin fractal.
pub fn jupiter_image() -> Image {
    let noise = Fbm::<Perlin>::new(JUPITER_NOISE_SEED);
    let mut pixels = vec![0u8; (JUPITER_WIDTH * JUPITER_HEIGHT * 4) as usize];
    let row_bytes = (JUPITER_WIDTH * 4) as usize;

    pixels.par_chunk_map_mut(ComputeTaskPool::get(), row_bytes, |row, chunk| {
        let lat_rad = latitude_at_row(row as u32);
        for column in 0..JUPITER_WIDTH {
            let lon_rad = longitude_at_column(column);
            let color = jupiter_pixel_color(&noise, lat_rad, lon_rad);
            let offset = (column * 4) as usize;
            chunk[offset..offset + 4].copy_from_slice(&to_rgba8(color));
        }
    });

    Image::new(
        Extent3d {
            width: JUPITER_WIDTH,
            height: JUPITER_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Gera a textura de brilho radial (256x256, RGBA8 sRGB, branca com alfa suave decaindo do
/// centro para a borda), usada tingida com cores diferentes em todos os brilhos da UI.
pub fn glow_image() -> Image {
    let mut pixels = vec![0u8; (GLOW_SIZE * GLOW_SIZE * 4) as usize];
    let row_bytes = (GLOW_SIZE * 4) as usize;

    pixels.par_chunk_map_mut(ComputeTaskPool::get(), row_bytes, |row, chunk| {
        for column in 0..GLOW_SIZE {
            let alpha = glow_alpha(column, row as u32);
            let offset = (column * 4) as usize;
            chunk[offset..offset + 4].copy_from_slice(&[
                255,
                255,
                255,
                (alpha * 255.0).round() as u8,
            ]);
        }
    });

    Image::new(
        Extent3d {
            width: GLOW_SIZE,
            height: GLOW_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Alfa radial suave de um pixel da textura de brilho: máximo no centro, zero na borda do
/// círculo inscrito no quadrado da textura.
fn glow_alpha(column: u32, row: u32) -> f32 {
    let center = (GLOW_SIZE - 1) as f32 / 2.0;
    let dx = (column as f32 - center) / center;
    let dy = (row as f32 - center) / center;
    let r = (dx * dx + dy * dy).sqrt();
    let f = (1.0 - r).max(0.0);
    (f * f * (0.35 + 0.65 * f)).clamp(0.0, 1.0)
}

/// Latitude em radianos de uma linha da textura: +90° no topo (linha 0) a -90° embaixo.
fn latitude_at_row(row: u32) -> f32 {
    (0.5 - row as f32 / (JUPITER_HEIGHT - 1) as f32) * PI
}

/// Longitude em radianos de uma coluna da textura, de 0 a 2π.
fn longitude_at_column(column: u32) -> f32 {
    column as f32 / (JUPITER_WIDTH - 1) as f32 * TAU
}

/// Ponto 3D correspondente a uma latitude/longitude na esfera unitária, usado como coordenada de
/// amostragem do ruído para não haver costura nas bordas da textura.
fn sphere_point(lat_rad: f32, lon_rad: f32) -> Vec3 {
    let (lat_sin, lat_cos) = lat_rad.sin_cos();
    let (lon_sin, lon_cos) = lon_rad.sin_cos();
    Vec3::new(lat_cos * lon_cos, lat_cos * lon_sin, lat_sin)
}

/// Ruído normalizado para o intervalo [0, 1].
fn noise01(noise: &Fbm<Perlin>, x: f32, y: f32, z: f32) -> f32 {
    (noise.get([x as f64, y as f64, z as f64]) as f32 * 0.5 + 0.5).clamp(0.0, 1.0)
}

/// Decodifica uma cor `0xRRGGBB` para um vetor RGB normalizado em [0, 1].
fn hex_rgb(hex: u32) -> Vec3 {
    Vec3::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

/// Normaliza um ângulo para o intervalo [-π, π].
fn wrap_to_pi(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Cor da faixa de Júpiter numa latitude (graus), interpolando a tabela `BAND_COLORS`.
fn band_color(lat_deg: f32) -> Vec3 {
    let first = BAND_COLORS[0];
    let last = BAND_COLORS[BAND_COLORS.len() - 1];

    if lat_deg <= first.0 {
        return hex_rgb(first.1);
    }
    if lat_deg >= last.0 {
        return hex_rgb(last.1);
    }

    for window in BAND_COLORS.windows(2) {
        let (lower_lat, lower_hex) = window[0];
        let (upper_lat, upper_hex) = window[1];
        if lat_deg <= upper_lat {
            let t = smoothstep(lower_lat, upper_lat, lat_deg);
            return hex_rgb(lower_hex).lerp(hex_rgb(upper_hex), t);
        }
    }

    hex_rgb(last.1)
}

/// Mistura a Grande Mancha Vermelha sobre a cor de base, centrada em
/// `GREAT_RED_SPOT_LAT_DEG`/`GREAT_RED_SPOT_LON_RAD`.
fn apply_great_red_spot(
    color: Vec3,
    noise: &Fbm<Perlin>,
    lat_rad: f32,
    lon_rad: f32,
    point: Vec3,
) -> Vec3 {
    let lat_spot_rad = GREAT_RED_SPOT_LAT_DEG.to_radians();
    let dl = wrap_to_pi(lon_rad - GREAT_RED_SPOT_LON_RAD);
    let g = (dl * lat_rad.cos() / 0.17).powi(2)
        + ((lat_rad - lat_spot_rad) / 0.07).powi(2)
        + (noise01(noise, point.x * 14.0, point.y * 14.0, point.z * 14.0) - 0.5) * 0.6;

    if g < 1.0 {
        let centrality = (1.0 - g).clamp(0.0, 1.0);
        let inner = hex_rgb(0xb4553a).lerp(hex_rgb(0xdc9670), centrality);
        color.lerp(inner, smoothstep(1.0, 0.7, g))
    } else if g < 1.8 {
        color.lerp(hex_rgb(0xf0e2c9), 0.35 * smoothstep(1.8, 1.0, g))
    } else {
        color
    }
}

/// Mistura as ovais brancas sobre a cor de base.
fn apply_white_ovals(mut color: Vec3, lat_rad: f32, lon_rad: f32) -> Vec3 {
    for index in 0..WHITE_OVAL_COUNT {
        let lon_center = index as f32 * 1.25 + 0.4;
        let dl = wrap_to_pi(lon_rad - lon_center);
        let o =
            (dl * lat_rad.cos() / 0.05).powi(2) + ((lat_rad - WHITE_OVAL_LAT_RAD) / 0.025).powi(2);
        color = color.lerp(hex_rgb(0xf5efe2), smoothstep(1.0, 0.4, o));
    }
    color
}

/// Mistura as manchas escuras perto do equador sobre a cor de base.
fn apply_dark_spots(mut color: Vec3, lat_rad: f32, lon_rad: f32) -> Vec3 {
    for index in 0..DARK_SPOT_COUNT {
        let lon_center = index as f32 * 0.9 + 0.2;
        let dl = wrap_to_pi(lon_rad - lon_center);
        let o =
            (dl * lat_rad.cos() / 0.06).powi(2) + ((lat_rad - DARK_SPOT_LAT_RAD) / 0.018).powi(2);
        color = color.lerp(hex_rgb(0x6f6a72), 0.55 * smoothstep(1.0, 0.3, o));
    }
    color
}

/// Cor final de um pixel da textura de Júpiter, numa latitude/longitude dadas.
fn jupiter_pixel_color(noise: &Fbm<Perlin>, lat_rad: f32, lon_rad: f32) -> Vec3 {
    let point = sphere_point(lat_rad, lon_rad);

    let band_distortion = (noise01(noise, point.x * 2.0, point.y * 2.0, point.z * 7.0) - 0.5) * 9.0
        + (noise01(noise, point.x * 5.0 + 3.0, point.y * 5.0, point.z * 22.0) - 0.5) * 3.5;
    let mut color = band_color(lat_rad.to_degrees() + band_distortion);

    let streaks = 1.0 + (noise01(noise, point.x * 8.0, point.y * 8.0, point.z * 70.0) - 0.5) * 0.32;
    color *= streaks;

    color = apply_great_red_spot(color, noise, lat_rad, lon_rad, point);
    color = apply_white_ovals(color, lat_rad, lon_rad);
    color = apply_dark_spots(color, lat_rad, lon_rad);

    color
}

/// Converte uma cor RGB em [0, 1] (possivelmente fora da faixa) para bytes RGBA8 opacos.
fn to_rgba8(color: Vec3) -> [u8; 4] {
    [
        (color.x.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.y.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.z.clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::tasks::TaskPool;

    fn ensure_task_pool() {
        ComputeTaskPool::get_or_init(TaskPool::default);
    }

    #[test]
    fn hex_rgb_decodes_known_colors() {
        assert_eq!(hex_rgb(0xffffff), Vec3::ONE);
        assert_eq!(hex_rgb(0x000000), Vec3::ZERO);
        assert_eq!(hex_rgb(0xff0000), Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn band_color_matches_table_endpoints() {
        assert_eq!(band_color(-90.0), hex_rgb(0x8a8072));
        assert_eq!(band_color(-100.0), hex_rgb(0x8a8072));
        assert_eq!(band_color(90.0), hex_rgb(0x887e70));
        assert_eq!(band_color(100.0), hex_rgb(0x887e70));
    }

    #[test]
    fn band_color_interpolates_between_entries() {
        let midpoint = band_color(-54.0);
        assert_ne!(midpoint, hex_rgb(0xab9c86));
        assert_ne!(midpoint, hex_rgb(0xcbb99a));
    }

    #[test]
    fn wrap_to_pi_keeps_angles_in_range() {
        for angle in [0.0, PI, -PI, TAU, 3.0 * PI, -3.0 * PI] {
            let wrapped = wrap_to_pi(angle);
            assert!((-PI..=PI).contains(&wrapped), "{wrapped} fora de [-pi, pi]");
        }
    }

    #[test]
    fn sphere_point_is_on_the_unit_sphere() {
        for (lat, lon) in [(0.0, 0.0), (PI / 4.0, PI), (-PI / 2.0, 1.0)] {
            let point = sphere_point(lat, lon);
            assert!((point.length() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn great_red_spot_tints_its_center_differently_from_a_distant_point() {
        let noise = Fbm::<Perlin>::new(JUPITER_NOISE_SEED);
        let spot_lat = GREAT_RED_SPOT_LAT_DEG.to_radians();
        let at_spot = jupiter_pixel_color(&noise, spot_lat, GREAT_RED_SPOT_LON_RAD);
        let far_away = jupiter_pixel_color(&noise, spot_lat, GREAT_RED_SPOT_LON_RAD + PI / 2.0);
        assert!((at_spot - far_away).length() > 0.05);
    }

    #[test]
    fn glow_alpha_is_near_maximal_at_the_four_center_pixels_and_zero_at_corners() {
        // GLOW_SIZE é par, então o centro contínuo cai entre pixels; não existe um pixel
        // exatamente no pico, só os quatro mais próximos dele.
        let center = (GLOW_SIZE - 1) / 2;
        for (column, row) in [
            (center, center),
            (center + 1, center),
            (center, center + 1),
            (center + 1, center + 1),
        ] {
            let alpha = glow_alpha(column, row);
            assert!(
                alpha > 0.95,
                "{alpha} deveria estar perto do pico no centro"
            );
        }
        assert_eq!(glow_alpha(0, 0), 0.0);
        assert_eq!(glow_alpha(GLOW_SIZE - 1, 0), 0.0);
    }

    #[test]
    fn glow_alpha_decreases_monotonically_from_the_center() {
        let center = (GLOW_SIZE - 1) / 2;
        let near = glow_alpha(center + GLOW_SIZE / 8, center);
        let far = glow_alpha(center + GLOW_SIZE / 4, center);
        assert!(near > far);
        assert!(far >= 0.0);
    }

    #[test]
    fn glow_image_has_expected_dimensions_and_format() {
        ensure_task_pool();
        let image = glow_image();
        assert_eq!(image.texture_descriptor.size.width, GLOW_SIZE);
        assert_eq!(image.texture_descriptor.size.height, GLOW_SIZE);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
    }

    #[test]
    fn jupiter_image_has_expected_dimensions_and_format() {
        ensure_task_pool();
        let image = jupiter_image();
        assert_eq!(image.texture_descriptor.size.width, JUPITER_WIDTH);
        assert_eq!(image.texture_descriptor.size.height, JUPITER_HEIGHT);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some((JUPITER_WIDTH * JUPITER_HEIGHT * 4) as usize)
        );
    }
}
