//! Mapa de cor do terreno: a cor da receita assada numa textura que cobre o recorte inteiro.
//!
//! A cor por vértice perde resolução junto com a malha: no nível mais grosso do LOD o espaçamento
//! é de ~43 m, e as bordas de cor (paterae, manchas de geada) viram serrilhado que muda de forma
//! quando o chunk troca de nível. O mapa guarda a cor com resolução fixa, independente do nível.
//!
//! O mapa é assado fora da thread principal. Até ficar pronto, o terreno mostra a cor por vértice,
//! que continua na malha e é também o que o anel de extensão usa fora do recorte.

use crate::height::{ColorFn, ColorSource};
use crate::surface_material::TerrainMaterial;
use crate::surface_mips::{MipKind, build_mip_chain, linear_to_srgb};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::{Assets, Commands, Handle, Image, ResMut, Resource};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::thread;

/// Anisotropia máxima do sampler do mapa. O terreno é visto quase rasante, onde o mip isotrópico
/// borraria a cor na direção da profundidade.
const ANISOTROPY_CLAMP: u16 = 16;

/// Resolução do mapa de cor de uma receita.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorMapConfig {
    /// Texels por lado. O mapa cobre o recorte quadrado inteiro (`TerrainParams::size`).
    pub texels_per_side: u32,
}

/// Mapa de cor sendo assado em segundo plano para o material do terreno. Removido quando o mapa
/// fica pronto ou ao sair de `Running` (descartar a tarefa a cancela se ela ainda não começou).
#[derive(Resource)]
pub(crate) struct PendingColorMap {
    material: Handle<TerrainMaterial>,
    terrain_size: f32,
    task: Task<Image>,
}

/// Começa a assar o mapa de cor de `color` para o recorte de lado `terrain_size` e agenda a troca
/// no `material` quando ele ficar pronto (ver [`finish_color_map`]).
pub(crate) fn start_color_map(
    commands: &mut Commands,
    color: ColorFn,
    terrain_size: f32,
    config: ColorMapConfig,
    material: Handle<TerrainMaterial>,
) {
    let texels = config.texels_per_side;
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let base = bake_color_texels(color.as_ref(), terrain_size, texels);
        color_map_image(&base, texels)
    });
    commands.insert_resource(PendingColorMap {
        material,
        terrain_size,
        task,
    });
}

/// Sistema que, quando o mapa termina de assar, o registra e liga no material do terreno.
pub(crate) fn finish_color_map(
    mut commands: Commands,
    mut pending: ResMut<PendingColorMap>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let Some(image) = block_on(poll_once(&mut pending.task)) else {
        return;
    };
    let image = images.add(image);
    if let Some(material) = materials.get_mut(&pending.material) {
        material
            .extension
            .enable_color_map(image, pending.terrain_size);
    }
    commands.remove_resource::<PendingColorMap>();
}

/// Amostra `color` no centro de cada texel de uma grade `texels` x `texels` que cobre o quadrado
/// de lado `terrain_size` centrado na origem. Devolve RGBA8 com a cor em sRGB, linha a linha, com
/// a linha 0 no `z` mais negativo e a coluna 0 no `x` mais negativo. As linhas são divididas entre
/// todos os núcleos: o mapa de Io leva ~7 s numa thread só.
pub(crate) fn bake_color_texels(
    color: &dyn ColorSource,
    terrain_size: f32,
    texels: u32,
) -> Vec<u8> {
    let side = texels as usize;
    let texel_size = terrain_size / texels as f32;
    let half = terrain_size * 0.5;
    let mut data = vec![0u8; side * side * 4];
    if side == 0 {
        return data;
    }

    let threads = thread::available_parallelism().map_or(1, |count| count.get());
    let rows_per_band = side.div_ceil(threads);
    thread::scope(|scope| {
        for (band, band_data) in data.chunks_mut(rows_per_band * side * 4).enumerate() {
            scope.spawn(move || {
                for (index, texel) in band_data.chunks_exact_mut(4).enumerate() {
                    let row = band * rows_per_band + index / side;
                    let column = index % side;
                    let x = -half + (column as f32 + 0.5) * texel_size;
                    let z = -half + (row as f32 + 0.5) * texel_size;
                    texel.copy_from_slice(&encode_srgba8(color.color_at(x, z)));
                }
            });
        }
    });
    data
}

/// Converte uma cor RGBA linear em RGBA8 com o RGB em sRGB (o alfa fica linear).
fn encode_srgba8([r, g, b, a]: [f32; 4]) -> [u8; 4] {
    let to_byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        to_byte(linear_to_srgb(r.clamp(0.0, 1.0))),
        to_byte(linear_to_srgb(g.clamp(0.0, 1.0))),
        to_byte(linear_to_srgb(b.clamp(0.0, 1.0))),
        to_byte(a),
    ]
}

/// Monta a imagem do mapa (sRGB, cadeia de mips completa, sampler linear anisotrópico com
/// `ClampToEdge`, o padrão). Só o mundo de render guarda os bytes: a CPU não lê o mapa de volta.
fn color_map_image(base: &[u8], texels: u32) -> Image {
    let (data, levels) = build_mip_chain(base, texels, texels, MipKind::SrgbColor);
    let mut image = Image::new_uninit(
        Extent3d {
            width: texels,
            height: texels,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        anisotropy_clamp: ANISOTROPY_CLAMP,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cor que codifica a posição: vermelho cresce com `x`, verde com `z`, ambos de 0 a 1 no
    /// quadrado de lado 4 centrado na origem.
    struct PositionColor;

    impl ColorSource for PositionColor {
        fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
            [(x + 2.0) / 4.0, (z + 2.0) / 4.0, 0.0, 1.0]
        }
    }

    fn texel(data: &[u8], texels: u32, column: u32, row: u32) -> [u8; 4] {
        let offset = ((row * texels + column) * 4) as usize;
        data[offset..offset + 4]
            .try_into()
            .expect("4 bytes por texel")
    }

    /// Testa se cada texel é a cor no seu centro, com a coluna seguindo `x` e a linha seguindo `z`.
    #[test]
    fn each_texel_holds_the_color_at_its_center() {
        let data = bake_color_texels(&PositionColor, 4.0, 4);

        // centros em -1,5, -0,5, 0,5 e 1,5 m: vermelho linear 0,125 na coluna 0, 0,875 na 3
        assert_eq!(
            texel(&data, 4, 0, 0),
            encode_srgba8([0.125, 0.125, 0.0, 1.0])
        );
        assert_eq!(
            texel(&data, 4, 3, 0),
            encode_srgba8([0.875, 0.125, 0.0, 1.0])
        );
        assert_eq!(
            texel(&data, 4, 0, 3),
            encode_srgba8([0.125, 0.875, 0.0, 1.0])
        );
    }

    /// Testa se a divisão em faixas por thread cobre todas as linhas, inclusive quando o número
    /// de linhas não é múltiplo do número de threads.
    #[test]
    fn bands_cover_every_row_when_rows_do_not_divide_evenly() {
        let texels = 37;

        let data = bake_color_texels(&PositionColor, 4.0, texels);

        assert_eq!(data.len(), (texels * texels * 4) as usize);
        for row in 0..texels {
            let z = -2.0 + (row as f32 + 0.5) * 4.0 / texels as f32;
            let expected = encode_srgba8([0.0, (z + 2.0) / 4.0, 0.0, 1.0])[1];
            assert_eq!(texel(&data, texels, 0, row)[1], expected, "linha {row}");
        }
    }

    /// Testa se a cor é guardada em sRGB: o cinza linear 0,214 vira o código ~128, não ~55.
    #[test]
    fn colors_are_stored_in_srgb() {
        let [r, _, _, a] = encode_srgba8([0.214, 0.0, 0.0, 1.0]);

        assert!((i32::from(r) - 128).abs() <= 1, "obtido {r}");
        assert_eq!(a, 255);
    }

    /// Testa se a imagem do mapa é sRGB e tem a cadeia de mips completa.
    #[test]
    fn map_image_is_srgb_with_a_full_mip_chain() {
        let base = bake_color_texels(&PositionColor, 4.0, 8);

        let image = color_map_image(&base, 8);

        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(image.texture_descriptor.mip_level_count, 4);
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some((64 + 16 + 4 + 1) * 4)
        );
    }
}
