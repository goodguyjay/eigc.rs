//! Valida os quatro perfis reais em `assets/moons/`: desserializam, têm o `MoonId` certo e o
//! tamanho angular de Júpiter bate com o critério de aceite da issue #8.

use eigc_moons::profile::{MoonId, MoonProfile};
use rstest::rstest;
use std::path::PathBuf;

/// Lê e desserializa `assets/moons/<file_name>` direto com `ron`, sem `AssetServer`.
fn load_profile(file_name: &str) -> MoonProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/moons")
        .join(file_name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("não consegui ler {}: {err}", path.display()));
    ron::from_str(&content)
        .unwrap_or_else(|err| panic!("{} não desserializa: {err}", path.display()))
}

/// Cada perfil desserializa, aponta para a lua certa e tem o diâmetro angular da issue #8.
#[rstest]
#[case::io("io.ron", MoonId::Io, 18.8)]
#[case::europa("europa.ron", MoonId::Europa, 11.9)]
#[case::ganymede("ganymede.ron", MoonId::Ganymede, 7.5)]
#[case::callisto("callisto.ron", MoonId::Callisto, 4.3)]
fn profile_has_expected_jupiter_angular_diameter(
    #[case] file_name: &str,
    #[case] expected_id: MoonId,
    #[case] expected_deg: f32,
) {
    let profile = load_profile(file_name);

    assert_eq!(profile.moon_id, expected_id);
    assert!(
        (profile.jupiter_angular_diameter_deg - expected_deg).abs() < 1e-4,
        "{file_name}: esperado {expected_deg}°, encontrado {}°",
        profile.jupiter_angular_diameter_deg
    );
}

/// `sky.jupiter_ang_radius` (usado no eclipse) e `jupiter_angular_diameter_deg` (usado no
/// disco) guardam a mesma grandeza em unidades diferentes; este teste impede que divirjam.
#[rstest]
#[case::io("io.ron")]
#[case::europa("europa.ron")]
#[case::ganymede("ganymede.ron")]
#[case::callisto("callisto.ron")]
fn jupiter_ang_radius_matches_half_angular_diameter(#[case] file_name: &str) {
    let profile = load_profile(file_name);

    let expected_rad = (profile.jupiter_angular_diameter_deg * 0.5).to_radians();
    assert!(
        (profile.sky.jupiter_ang_radius - expected_rad).abs() < 1e-6,
        "{file_name}: jupiter_ang_radius = {}, esperado {expected_rad}",
        profile.sky.jupiter_ang_radius
    );
}
