//! Geração de mipmaps em CPU para as texturas de superfície do terreno.

use crate::surface_textures::TerrainSurfaceTextures;
use bevy::prelude::{Assets, Image, Res, ResMut};
use bevy::render::render_resource::TextureFormat;

/// Como os canais de uma textura devem ser combinados ao reduzir a resolução.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MipKind {
    /// Cor em sRGB
    SrgbColor,
    /// Dados lineares (ex.: ARM)
    LinearData,
    /// Média dos vetores
    NormalMap,
}

/// Gera a cadeia de mips das texturas de superfície assim que cada uma termina de carregar.
pub(crate) fn generate_surface_mipmaps(
    textures: Res<TerrainSurfaceTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    let targets = [
        (&textures.diffuse, MipKind::SrgbColor),
        (&textures.normal, MipKind::NormalMap),
        (&textures.arm, MipKind::LinearData),
    ];

    for (handle, kind) in targets {
        let needs_mips = images.get(handle).is_some_and(|image| {
            image.texture_descriptor.mip_level_count == 1
                && image.data.is_some()
                && is_rgba8(image.texture_descriptor.format)
        });
        if !needs_mips {
            continue;
        }
        let Some(image) = images.get_mut(handle) else {
            continue;
        };
        let (width, height) = (image.width(), image.height());
        let Some(base) = image.data.as_deref() else {
            continue;
        };

        let (data, levels) = build_mip_chain(base, width, height, kind);
        image.data = Some(data);
        image.texture_descriptor.mip_level_count = levels;
    }
}

/// Retorna `true` se o formato for RGBA8 (sRGB ou linear).
fn is_rgba8(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    )
}

/// Número de níveis de uma cadeia completa até 1x1.
fn mip_level_count(width: u32, height: u32) -> u32 {
    width.max(height).max(1).ilog2() + 1
}

/// Devolve os bytes `nível 0 + nível 1 + ...` (RGBA8, um após o outro) e a quantidade de níveis.
fn build_mip_chain(base: &[u8], width: u32, height: u32, kind: MipKind) -> (Vec<u8>, u32) {
    let levels = mip_level_count(width, height);
    let mut data = Vec::with_capacity(base.len() + base.len() / 3);
    data.extend_from_slice(base);

    let (mut level_start, mut level_width, mut level_height) = (0usize, width, height);
    for _ in 1..levels {
        let next_width = (level_width / 2).max(1);
        let next_height = (level_height / 2).max(1);
        let level_end = level_start + (level_width * level_height * 4) as usize;

        let mut next = Vec::with_capacity((next_width * next_height * 4) as usize);
        for y in 0..next_height {
            for x in 0..next_width {
                next.extend_from_slice(&downsample_pixel(
                    &data[level_start..level_end],
                    (level_width, level_height),
                    (x, y),
                    kind,
                ));
            }
        }

        data.extend_from_slice(&next);
        level_start = level_end;
        level_width = next_width;
        level_height = next_height;
    }

    (data, levels)
}

/// Combina o bloco 2x2 de origem que cobre o pixel `(x, y)` do nível seguinte.
fn downsample_pixel(
    source: &[u8],
    (width, height): (u32, u32),
    (x, y): (u32, u32),
    kind: MipKind,
) -> [u8; 4] {
    let mut texels = [[0u8; 4]; 4];
    for (index, texel) in texels.iter_mut().enumerate() {
        let sx = (x * 2 + (index as u32 & 1)).min(width - 1);
        let sy = (y * 2 + (index as u32 >> 1)).min(height - 1);
        let offset = ((sy * width + sx) * 4) as usize;
        texel.copy_from_slice(&source[offset..offset + 4]);
    }

    let alpha = (texels.iter().map(|t| u32::from(t[3])).sum::<u32>() as f32 / 4.0).round() as u8;

    match kind {
        MipKind::LinearData => {
            let mut out = [0u8, 0, 0, alpha];
            for channel in 0..3 {
                let sum: u32 = texels.iter().map(|t| u32::from(t[channel])).sum();
                out[channel] = (sum as f32 / 4.0).round() as u8;
            }
            out
        }
        MipKind::SrgbColor => {
            let mut out = [0u8, 0, 0, alpha];
            for channel in 0..3 {
                let linear: f32 = texels
                    .iter()
                    .map(|t| srgb_to_linear(f32::from(t[channel]) / 255.0))
                    .sum::<f32>()
                    / 4.0;
                out[channel] = (linear_to_srgb(linear) * 255.0).round() as u8;
            }
            out
        }
        MipKind::NormalMap => {
            let mut sum = [0.0_f32; 3];
            for texel in &texels {
                for channel in 0..3 {
                    sum[channel] += f32::from(texel[channel]) / 255.0 * 2.0 - 1.0;
                }
            }
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            let unit = if length > 1e-4 {
                [sum[0] / length, sum[1] / length, sum[2] / length]
            } else {
                [0.0, 0.0, 1.0]
            };
            [
                ((unit[0] * 0.5 + 0.5) * 255.0).round() as u8,
                ((unit[1] * 0.5 + 0.5) * 255.0).round() as u8,
                ((unit[2] * 0.5 + 0.5) * 255.0).round() as u8,
                alpha,
            ]
        }
    }
}

/// Converte um valor de cor de sRGB para linear (0.0..1.0).
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Converte um valor de cor de linear para sRGB (0.0..1.0).
fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        rgba.repeat((width * height) as usize)
    }

    /// Testa se a cadeia de mips é construída corretamente até o nível 1x1.
    #[test]
    fn chain_has_all_levels_down_to_one_texel() {
        let base = solid(8, 8, [10, 20, 30, 255]);

        let (data, levels) = build_mip_chain(&base, 8, 8, MipKind::LinearData);

        assert_eq!(levels, 4);
        // 8x8 + 4x4 + 2x2 + 1x1 texels, 4 bytes cada
        assert_eq!(data.len(), (64 + 16 + 4 + 1) * 4);
        assert_eq!(&data[..base.len()], base.as_slice());
    }

    /// Testa se a média linear de 4 valores é calculada corretamente.
    #[test]
    fn linear_data_mip_is_the_plain_average() {
        let mut base = Vec::new();
        for r in [0u8, 100, 100, 200] {
            base.extend_from_slice(&[r, 0, 0, 255]);
        }

        let (data, _) = build_mip_chain(&base, 2, 2, MipKind::LinearData);

        assert_eq!(&data[16..20], &[100, 0, 0, 255]);
    }

    /// Testa se a média de 4 valores sRGB é calculada corretamente em espaço linear.
    #[test]
    fn srgb_mip_averages_in_linear_space_so_black_and_white_do_not_give_mid_gray_code() {
        let mut base = Vec::new();
        for value in [0u8, 255, 0, 255] {
            base.extend_from_slice(&[value, value, value, 255]);
        }

        let (data, _) = build_mip_chain(&base, 2, 2, MipKind::SrgbColor);

        assert!((i32::from(data[16]) - 188).abs() <= 1, "obtido {}", data[16]);
    }

    /// Testa se a média de 4 normais unitárias é normalizada e se normais opostas se cancelam.
    #[test]
    fn normal_map_mip_stays_unit_length_and_cancels_opposite_tilts() {
        // duas normais inclinadas para +X e duas para -X: a média aponta para +Z
        let tilt = |x: f32| {
            let z = (1.0_f32 - x * x).sqrt();
            [
                ((x * 0.5 + 0.5) * 255.0).round() as u8,
                128,
                ((z * 0.5 + 0.5) * 255.0).round() as u8,
                255,
            ]
        };
        let mut base = Vec::new();
        for x in [0.6, -0.6, 0.6, -0.6] {
            base.extend_from_slice(&tilt(x));
        }

        let (data, _) = build_mip_chain(&base, 2, 2, MipKind::NormalMap);

        let mip = &data[16..20];
        assert!((i32::from(mip[0]) - 128).abs() <= 1);
        assert!(mip[2] >= 254);
    }

    /// Testa se a cadeia de mips de uma textura não quadrada termina com 1 texel na menor dimensão.
    #[test]
    fn non_square_chain_clamps_the_short_side_at_one_texel() {
        let base = solid(4, 1, [1, 2, 3, 4]);

        let (data, levels) = build_mip_chain(&base, 4, 1, MipKind::LinearData);

        assert_eq!(levels, 3);
        assert_eq!(data.len(), (4 + 2 + 1) * 4);
    }
}
