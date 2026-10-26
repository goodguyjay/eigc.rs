use eigc_terrain::height::HeightSource;
use eigc_terrain::height::heightmap::{HeightmapError, HeightmapHeight};
use image::{ImageBuffer, ImageFormat, Luma};
use std::io::Cursor;

/// Elevação mínima documentada no `.ron` do heightmap de Europa, em metros.
const EUROPA_ELEVATION_MIN_M: f32 = -513.57763671875;

/// Elevação máxima documentada no `.ron` do heightmap de Europa, em metros.
const EUROPA_ELEVATION_MAX_M: f32 = -64.11154174804688;

/// Metadados RON mínimos para a grade 2x2 de teste (elevação de -100 m a 100 m, 1000 m de lado).
const CHECKER_RON: &str = r#"(
    source: "fixture",
    patch_width_m: 1000.0,
    patch_height_m: 1000.0,
    elevation_min_m: -100.0,
    elevation_max_m: 100.0,
)"#;

/// Codifica uma grade 2x2 de 16 bits (0, 65535 / 65535, 0) como PNG em memória.
fn checker_png_bytes() -> Vec<u8> {
    let buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
        ImageBuffer::from_raw(2, 2, vec![0, 65535, 65535, 0]).expect("buffer 2x2 deveria caber");
    let mut bytes = Cursor::new(Vec::new());
    buffer
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("png em memoria deveria ser gravavel");
    bytes.into_inner()
}

/// PNG e RON válidos reconstroem a altura em metros, ignorando campos de metadados extras.
#[test]
fn png_and_ron_rebuild_the_height_in_meters() {
    let heightmap = HeightmapHeight::from_png_and_ron(&checker_png_bytes(), CHECKER_RON)
        .expect("png e ron validos deveriam carregar");

    assert!((heightmap.height_at(-500.0, -500.0) + 100.0).abs() < 1e-3);
    assert!((heightmap.height_at(500.0, -500.0) - 100.0).abs() < 1e-3);
}

/// Bytes que não são um PNG viram erro de decodificação, não pânico.
#[test]
fn invalid_png_bytes_are_reported_as_a_decode_error() {
    let result = HeightmapHeight::from_png_and_ron(b"isto nao e um png", CHECKER_RON);
    assert!(matches!(result, Err(HeightmapError::PngDecode(_))));
}

/// RON sem os campos obrigatórios vira erro de metadados, não pânico.
#[test]
fn incomplete_ron_is_reported_as_a_metadata_error() {
    let result = HeightmapHeight::from_png_and_ron(&checker_png_bytes(), "(patch_width_m: 1.0)");
    assert!(matches!(result, Err(HeightmapError::MetadataParse(_))));
}

/// O heightmap real de Europa em `assets/terrain/europa` decodifica e toda amostra do recorte
/// cai dentro da faixa de elevação documentada no `.ron`, a menos do overshoot da bicúbica (o
/// PNG não saiu corrompido nem com profundidade de bits errada).
#[test]
fn shipped_europa_heightmap_decodes_within_its_documented_elevation_range() {
    let png =
        include_bytes!("../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.png");
    let ron =
        include_str!("../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.ron");

    let heightmap = HeightmapHeight::from_png_and_ron(png, ron)
        .expect("heightmap de Europa em assets deveria carregar");

    let tolerance_m = 0.1 * (EUROPA_ELEVATION_MAX_M - EUROPA_ELEVATION_MIN_M);
    let half_extent_m = 11_000.0;
    let steps = 50;
    let mut lowest = f32::MAX;
    let mut highest = f32::MIN;
    for i in 0..=steps {
        for j in 0..=steps {
            let x = -half_extent_m + 2.0 * half_extent_m * i as f32 / steps as f32;
            let z = -half_extent_m + 2.0 * half_extent_m * j as f32 / steps as f32;
            let height = heightmap.height_at(x, z);
            lowest = lowest.min(height);
            highest = highest.max(height);
        }
    }

    assert!(
        lowest >= EUROPA_ELEVATION_MIN_M - tolerance_m,
        "mínimo fora da faixa: {lowest}"
    );
    assert!(
        highest <= EUROPA_ELEVATION_MAX_M + tolerance_m,
        "máximo fora da faixa: {highest}"
    );
    assert!(
        highest - lowest > 100.0,
        "o relevo real deveria variar bem mais que 100 m, variou {}",
        highest - lowest
    );
}
