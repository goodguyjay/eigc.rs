//! Fonte de altura baseada em um DTM real (heightmap 16 bits + metadados RON), amostrado por
//! interpolação bicúbica (Catmull-Rom).

use super::HeightSource;
use image::ImageFormat;
use serde::Deserialize;
use thiserror::Error;

/// Valor máximo de um pixel do heightmap de 16 bits, usado para desnormalizar para metros.
const HEIGHTMAP_PIXEL_MAX: f32 = 65535.0;

/// Metadados do heightmap lidos do `.ron` gerado por `tools/dtm_to_heightmap.py`. Só os campos
/// necessários para reconstruir a altura em metros.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HeightmapMetadata {
    /// Largura do recorte, em metros.
    pub patch_width_m: f32,
    /// Altura (extensão no eixo z) do recorte, em metros.
    pub patch_height_m: f32,
    /// Elevação real correspondente ao pixel `0`, em metros.
    pub elevation_min_m: f32,
    /// Elevação real correspondente ao pixel `65535`, em metros.
    pub elevation_max_m: f32,
}

/// Erros possíveis ao construir um [`HeightmapHeight`].
#[derive(Debug, Error)]
pub enum HeightmapError {
    /// O PNG não pôde ser decodificado.
    #[error("Falha ao decodificar o png do heightmap: {0}")]
    PngDecode(#[from] image::ImageError),
    /// Os metadados RON não puderam ser interpretados.
    #[error("Falha ao interpretar os metadados ron do heightmap: {0}")]
    MetadataParse(#[from] ron::de::SpannedError),
    /// A quantidade de pixels não bate com `width * height`, ou a grade é pequena demais para
    /// interpolar (menos de 2x2).
    #[error("Grade de heightmap invalida: {pixel_count} pixels para {width}x{height}")]
    InvalidGrid {
        /// Quantidade de pixels recebida.
        pixel_count: usize,
        /// Largura declarada, em pixels.
        width: usize,
        /// Altura declarada, em pixels.
        height: usize,
    },
}

/// Fonte de altura que amostra um DTM real por interpolação bicúbica (Catmull-Rom).
///
/// O recorte fica centrado na origem: `x` em `[-patch_width_m / 2, patch_width_m / 2]` e `z` em
/// `[-patch_height_m / 2, patch_height_m / 2]`, mesma convenção dos demais `HeightSource`. A
/// linha 0 do raster fica em `z` mínimo. Fora do recorte a altura repete a borda.
///
/// A interpolação é bicúbica, não bilinear. Cada pixel do DTM cobre dezenas de
/// quads da malha, e a bilinear tem gradiente descontínuo nas bordas dos pixels, o que as
/// normais e a cor por inclinação mostram como um tabuleiro. Catmull-Rom tem gradiente contínuo
/// e passa exatamente pelos pixels, mas pode ultrapassar levemente a faixa de elevação dos
/// vizinhos (overshoot). A altura
/// devolvida é a elevação real, em metros, sem nenhum deslocamento ou exagero vertical; isso é
/// responsabilidade de quem compõe a fonte (ver `Bias` e `Scale` em `height::comb`).
#[derive(Debug, Clone)]
pub struct HeightmapHeight {
    data: Vec<f32>,
    width: usize,
    height: usize,
    patch_width_m: f32,
    patch_height_m: f32,
}

impl HeightmapHeight {
    /// Cria a fonte a partir dos pixels crus de 16 bits (`0..=65535`, linha a linha), já
    /// desnormalizando para metros com `elevation_min_m`/`elevation_max_m`.
    pub fn new(
        raw_u16: &[u16],
        width: usize,
        height: usize,
        elevation_min_m: f32,
        elevation_max_m: f32,
        patch_width_m: f32,
        patch_height_m: f32,
    ) -> Result<Self, HeightmapError> {
        if width < 2 || height < 2 || raw_u16.len() != width * height {
            return Err(HeightmapError::InvalidGrid {
                pixel_count: raw_u16.len(),
                width,
                height,
            });
        }

        let range = elevation_max_m - elevation_min_m;
        let data = raw_u16
            .iter()
            .map(|&pixel| elevation_min_m + (pixel as f32 / HEIGHTMAP_PIXEL_MAX) * range)
            .collect();

        Ok(Self {
            data,
            width,
            height,
            patch_width_m,
            patch_height_m,
        })
    }

    /// Cria a fonte a partir do conteúdo de um PNG de 16 bits em escala de cinza e do texto do
    /// `.ron` de metadados correspondente.
    pub fn from_png_and_ron(png_bytes: &[u8], ron_text: &str) -> Result<Self, HeightmapError> {
        let metadata: HeightmapMetadata = ron::de::from_str(ron_text)?;
        let image = image::load_from_memory_with_format(png_bytes, ImageFormat::Png)?.into_luma16();
        let (width, height) = image.dimensions();

        Self::new(
            image.as_raw(),
            width as usize,
            height as usize,
            metadata.elevation_min_m,
            metadata.elevation_max_m,
            metadata.patch_width_m,
            metadata.patch_height_m,
        )
    }
}

impl HeightmapHeight {
    /// Elevação do pixel (`col`, `row`), repetindo a borda para índices fora da grade.
    fn texel(&self, col: isize, row: isize) -> f32 {
        let col = col.clamp(0, self.width as isize - 1) as usize;
        let row = row.clamp(0, self.height as isize - 1) as usize;
        self.data[row * self.width + col]
    }
}

/// Interpolação Catmull-Rom entre `p1` e `p2` (com `t` em `[0, 1]`), usando `p0` e `p3` para a
/// tangente. Passa exatamente pelos pontos e tem derivada contínua entre segmentos.
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t)
}

impl HeightSource for HeightmapHeight {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let u = (x / self.patch_width_m + 0.5).clamp(0.0, 1.0) * (self.width - 1) as f32;
        let v = (z / self.patch_height_m + 0.5).clamp(0.0, 1.0) * (self.height - 1) as f32;

        let col = u.floor() as isize;
        let row = v.floor() as isize;
        let frac_x = u - col as f32;
        let frac_z = v - row as f32;

        let rows: [f32; 4] = std::array::from_fn(|offset| {
            let r = row + offset as isize - 1;
            catmull_rom(
                self.texel(col - 1, r),
                self.texel(col, r),
                self.texel(col + 1, r),
                self.texel(col + 2, r),
                frac_x,
            )
        });
        catmull_rom(rows[0], rows[1], rows[2], rows[3], frac_z)
    }
}
