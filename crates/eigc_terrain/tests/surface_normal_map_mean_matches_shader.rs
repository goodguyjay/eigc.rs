use image::ImageFormat;
use std::path::{Path, PathBuf};

/// Diferença máxima aceita entre a constante do shader e a média medida, por componente.
const MEAN_TOLERANCE: f64 = 0.003;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_text(relative_path: &str) -> String {
    let path = workspace_root().join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("deveria ler {}: {error}", path.display()))
}

/// Linha que declara `name` (`const NAME...`), sem os comentários ao redor.
fn declaration_line<'a>(source: &'a str, name: &str) -> &'a str {
    let prefix = format!("const {name}");
    source
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("declaração de {name} não encontrada"))
}

/// Lê `NORMAL_MAP_MEAN_XY` do shader, na forma `vec2<f32>(x, y)`.
fn shader_normal_map_mean() -> [f64; 2] {
    let shader = read_text("assets/shaders/terrain_detail.wgsl");
    let line = declaration_line(&shader, "NORMAL_MAP_MEAN_XY");
    let arguments = line
        .rsplit_once("vec2<f32>(")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(arguments, _)| arguments)
        .unwrap_or_else(|| panic!("NORMAL_MAP_MEAN_XY deveria ser vec2<f32>(x, y): {line}"));
    let values: Vec<f64> = arguments
        .split(',')
        .map(|value| value.trim().parse().expect("componente deveria ser número"))
        .collect();
    assert_eq!(
        values.len(),
        2,
        "NORMAL_MAP_MEAN_XY deveria ter dois componentes"
    );
    [values[0], values[1]]
}

/// Lê o caminho do normal map que o jogo carrega (`EUROPA_ICE_NORMAL_PATH`, privado em
/// `surface_textures.rs`), para o teste acompanhar uma troca de textura.
fn loaded_normal_map_path() -> String {
    let source = read_text("crates/eigc_terrain/src/surface_textures.rs");
    let line = declaration_line(&source, "EUROPA_ICE_NORMAL_PATH");
    let path = line
        .split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("EUROPA_ICE_NORMAL_PATH deveria ser uma string: {line}"));
    format!("assets/{path}")
}

/// Média do xy da normal decodificada para [-1, 1], como o shader faz.
fn measured_normal_map_mean(relative_path: &str) -> [f64; 2] {
    let path = workspace_root().join(relative_path);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("deveria ler {}: {error}", path.display()));
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .expect("o normal map deveria ser um png decodificável")
        .into_rgb8();

    let mut sum = [0.0_f64; 2];
    for pixel in image.pixels() {
        for (channel, total) in sum.iter_mut().enumerate() {
            *total += f64::from(pixel[channel]) / 255.0 * 2.0 - 1.0;
        }
    }
    let count = f64::from(image.width()) * f64::from(image.height());
    [sum[0] / count, sum[1] / count]
}

/// A constante que o shader subtrai antes de girar a normal bate com a média do normal map que o
/// jogo de fato carrega. Trocar ou regerar a textura sem remedir `NORMAL_MAP_MEAN_XY` falha aqui.
#[test]
fn shader_normal_map_mean_matches_the_loaded_normal_map() {
    let normal_map_path = loaded_normal_map_path();
    let shader_mean = shader_normal_map_mean();
    let measured_mean = measured_normal_map_mean(&normal_map_path);

    for axis in 0..2 {
        assert!(
            (shader_mean[axis] - measured_mean[axis]).abs() <= MEAN_TOLERANCE,
            "NORMAL_MAP_MEAN_XY {shader_mean:?} não bate com a média medida {measured_mean:?} \
             de {normal_map_path}; usar o valor que tools/generate_europa_ice.py imprime"
        );
    }
}
