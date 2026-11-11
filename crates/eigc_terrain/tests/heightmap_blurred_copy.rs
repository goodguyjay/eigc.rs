//! Testes de integração da cópia borrada do heightmap (`HeightmapHeight::blurred`), usada como
//! "entorno" na cor por altura relativa.

use eigc_terrain::height::HeightSource;
use eigc_terrain::height::heightmap::HeightmapHeight;

/// Grade 9x9 com pixels de 100 m: tudo em 0 m, menos um pico de 900 m no centro.
fn single_peak_9x9() -> HeightmapHeight {
    let mut pixels = vec![0u16; 81];
    pixels[40] = 900;
    HeightmapHeight::new(&pixels, 9, 9, 0.0, 65535.0, 800.0, 800.0)
        .expect("grade 9x9 deveria ser valida")
}

/// Uma grade de altura constante continua constante depois do borrão, inclusive na borda.
#[test]
fn blurring_a_constant_grid_keeps_it_constant() {
    let constant = HeightmapHeight::new(&[500; 25], 5, 5, 0.0, 65535.0, 400.0, 400.0)
        .expect("grade 5x5 deveria ser valida");

    let blurred = constant.blurred(150.0);

    for (x, z) in [(0.0, 0.0), (-200.0, -200.0), (200.0, 130.0), (-37.0, 181.0)] {
        assert!((blurred.height_at(x, z) - 500.0).abs() < 1e-3);
    }
}

/// O borrão espalha um pico: o centro baixa, os vizinhos sobem, e a soma da elevação na grade
/// se conserva (longe da borda nada sai pela repetição dela).
#[test]
fn blurring_spreads_a_peak_and_keeps_the_total_elevation() {
    let original = single_peak_9x9();
    let blurred = original.blurred(100.0);

    let peak_before = original.height_at(0.0, 0.0);
    let peak_after = blurred.height_at(0.0, 0.0);
    assert!(
        peak_after < 0.5 * peak_before,
        "pico {peak_after} deveria baixar de {peak_before}"
    );
    assert!(blurred.height_at(100.0, 0.0) > 1.0, "vizinho deveria subir");

    let total = |source: &HeightmapHeight| -> f32 {
        (0..9)
            .flat_map(|row| (0..9).map(move |col| (col, row)))
            .map(|(col, row)| {
                source.height_at(col as f32 * 100.0 - 400.0, row as f32 * 100.0 - 400.0)
            })
            .sum()
    };
    assert!((total(&blurred) - total(&original)).abs() < 1.0);
}

/// Desvio zero não muda nada.
#[test]
fn zero_sigma_returns_the_same_heights() {
    let original = single_peak_9x9();
    let copy = original.blurred(0.0);

    for (x, z) in [(0.0, 0.0), (100.0, 0.0), (-250.0, 330.0)] {
        assert!((copy.height_at(x, z) - original.height_at(x, z)).abs() < 1e-3);
    }
}
