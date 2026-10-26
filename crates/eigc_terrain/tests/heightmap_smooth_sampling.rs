use eigc_terrain::height::HeightSource;
use eigc_terrain::height::heightmap::{HeightmapError, HeightmapHeight};
use rstest::rstest;

/// Tolerância para comparar alturas em metros após a conversão u16 -> f32.
const EPSILON_M: f32 = 1e-3;

/// Grade 2x2 em xadrez (0, 65535 / 65535, 0) com elevação de -100 m a 100 m, em um recorte de
/// 1000 m x 1000 m centrado na origem.
fn checker_2x2() -> HeightmapHeight {
    HeightmapHeight::new(&[0, 65535, 65535, 0], 2, 2, -100.0, 100.0, 1000.0, 1000.0)
        .expect("grade 2x2 deveria ser valida")
}

/// Grade 4x2 cujas linhas são idênticas (0, 20, 60, 70 m), com pixels de 100 m: toda a variação
/// é em x, e a derivada segunda é diferente de zero nos pixels internos.
fn bumpy_4x2() -> HeightmapHeight {
    HeightmapHeight::new(
        &[0, 20, 60, 70, 0, 20, 60, 70],
        4,
        2,
        0.0,
        65535.0,
        300.0,
        100.0,
    )
    .expect("grade 4x2 deveria ser valida")
}

/// Grade 3x2 em que o pixel vale a própria elevação em metros (10, 20, 30 / 40, 50, 60), em um
/// recorte de 200 m (x) por 100 m (z). Os valores distintos expõem eixos trocados.
fn ramp_3x2() -> HeightmapHeight {
    HeightmapHeight::new(&[10, 20, 30, 40, 50, 60], 3, 2, 0.0, 65535.0, 200.0, 100.0)
        .expect("grade 3x2 deveria ser valida")
}

/// Amostrar exatamente sobre um pixel de canto devolve a elevação desnormalizada desse pixel.
#[rstest]
#[case(-500.0, -500.0, -100.0)]
#[case(500.0, -500.0, 100.0)]
#[case(-500.0, 500.0, 100.0)]
#[case(500.0, 500.0, -100.0)]
fn sampling_exactly_on_a_grid_point_returns_the_pixel_value(
    #[case] x: f32,
    #[case] z: f32,
    #[case] expected_m: f32,
) {
    let height = checker_2x2().height_at(x, z);
    assert!(
        (height - expected_m).abs() < EPSILON_M,
        "altura em ({x}, {z}) deveria ser {expected_m}, veio {height}"
    );
}

/// No meio de uma célula a altura é a média dos quatro vizinhos.
#[test]
fn midpoint_of_a_cell_is_the_average_of_the_four_neighbours() {
    let height = checker_2x2().height_at(0.0, 0.0);
    assert!(
        height.abs() < EPSILON_M,
        "média de -100, 100, 100 e -100 deveria ser 0, veio {height}"
    );
}

/// Fora do recorte a altura repete a borda: não extrapola nem dá pânico.
#[rstest]
#[case(-999_999.0, -500.0, -500.0, -500.0)]
#[case(999_999.0, 500.0, 500.0, 500.0)]
#[case(-999_999.0, 999_999.0, -500.0, 500.0)]
#[case(f32::MAX, f32::MIN, 500.0, -500.0)]
fn coordinates_outside_the_patch_are_clamped_to_the_edge(
    #[case] x: f32,
    #[case] z: f32,
    #[case] edge_x: f32,
    #[case] edge_z: f32,
) {
    let heightmap = checker_2x2();
    let outside = heightmap.height_at(x, z);
    let on_edge = heightmap.height_at(edge_x, edge_z);
    assert!(outside.is_finite());
    assert!(
        (outside - on_edge).abs() < EPSILON_M,
        "fora do recorte deveria valer a borda: {outside} vs {on_edge}"
    );
}

/// `x` percorre as colunas e `z` as linhas do raster, com a linha 0 em `z` mínimo e a origem no
/// centro do recorte.
#[rstest]
#[case(-100.0, -50.0, 10.0)]
#[case(100.0, -50.0, 30.0)]
#[case(-100.0, 50.0, 40.0)]
#[case(100.0, 50.0, 60.0)]
#[case(0.0, 0.0, 35.0)]
fn x_walks_columns_and_z_walks_rows_with_origin_at_the_patch_center(
    #[case] x: f32,
    #[case] z: f32,
    #[case] expected_m: f32,
) {
    let height = ramp_3x2().height_at(x, z);
    assert!(
        (height - expected_m).abs() < EPSILON_M,
        "altura em ({x}, {z}) deveria ser {expected_m}, veio {height}"
    );
}

/// Quantidade de pixels incompatível com a grade, ou grade menor que 2x2, é rejeitada em vez de
/// causar acesso fora dos limites depois.
#[rstest]
#[case(&[0, 1, 2], 2, 2)]
#[case(&[0, 1, 2, 3], 1, 4)]
#[case(&[0, 1], 2, 1)]
#[case(&[], 0, 0)]
fn inconsistent_grid_is_rejected(
    #[case] pixels: &[u16],
    #[case] width: usize,
    #[case] height: usize,
) {
    let result = HeightmapHeight::new(pixels, width, height, 0.0, 1.0, 10.0, 10.0);
    assert!(matches!(result, Err(HeightmapError::InvalidGrid { .. })));
}

/// Regressão do "tabuleiro": a inclinação logo antes e logo depois da borda de um pixel é a
/// mesma. Com interpolação bilinear a inclinação saltava ali (aqui ~0,2 m/m), e as normais da
/// malha e a cor por inclinação desenhavam a grade de pixels do DTM no terreno.
#[test]
fn slope_is_continuous_across_pixel_boundaries() {
    let heightmap = bumpy_4x2();
    // Pixels de 100 m em um recorte de 300 m: o pixel de coluna 1 fica em x = -50.
    let boundary_x = -50.0;
    let probe = 0.5;
    let slope_before = (heightmap.height_at(boundary_x, 0.0)
        - heightmap.height_at(boundary_x - probe, 0.0))
        / probe;
    let slope_after = (heightmap.height_at(boundary_x + probe, 0.0)
        - heightmap.height_at(boundary_x, 0.0))
        / probe;

    assert!(
        (slope_before - slope_after).abs() < 0.02,
        "a inclinação deveria ser contínua na borda do pixel: {slope_before} vs {slope_after}"
    );
}

/// A interpolação passa exatamente pelos pixels, inclusive os internos (onde a bicúbica usa
/// vizinhos dos dois lados).
#[rstest]
#[case(-150.0, 0.0)]
#[case(-50.0, 20.0)]
#[case(50.0, 60.0)]
#[case(150.0, 70.0)]
fn interpolation_passes_through_interior_pixels(#[case] x: f32, #[case] expected_m: f32) {
    let height = bumpy_4x2().height_at(x, 0.0);
    assert!(
        (height - expected_m).abs() < EPSILON_M,
        "altura em x={x} deveria ser {expected_m}, veio {height}"
    );
}
