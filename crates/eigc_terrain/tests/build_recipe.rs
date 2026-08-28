//! Testes de integração para recipe::build_recipe. Cobre as quatro luas do enum MoonId

use eigc_moons::profile::{MoonId, MoonProfile, TerrainCalibration};
use eigc_terrain::recipe::build_recipe;
use rstest::rstest;

/// Cria um MoonProfile mínimo para teste, com valores arbitrários, exceto pelo moon_id.
fn minimal_profile_for(moon_id: MoonId) -> MoonProfile {
    MoonProfile {
        moon_id,
        display_name: "Lua teste".to_string(),
        jupiter_angular_diameter_deg: 10.0,
        terrain: TerrainCalibration {
            seed: 99,
            base_frequency: 0.001,
            feature_direction: [1.0, 0.0],
            vertical_amplitude_meters: 10.0,
            warp_amplitude_meters: 20.0,
            perceptual_roughness: 0.5,
            reflectance: 0.3,
        },
        terrain_base_color: [1.0, 1.0, 1.0, 1.0],
        walkable: true,
    }
}

/// Testa se a receita para Europa produz uma função de altura finita e parâmetros correspondentes ao perfil.
#[test]
fn europa_recipe_produces_height_function_and_matching_params() {
    let profile = minimal_profile_for(MoonId::Europa);
    let recipe = build_recipe(&profile);

    assert_eq!(recipe.params.seed, profile.terrain.seed);
    assert_eq!(recipe.params.amp, profile.terrain.vertical_amplitude_meters);
    assert_eq!(recipe.appearance.display_name, profile.display_name);

    assert_eq!(
        recipe.material_properties.perceptual_roughness,
        profile.terrain.perceptual_roughness,
        "perceptual_roughness não corresponde ao perfil"
    );

    let sample_height = recipe.height.height_at(100.0, 200.0);
    assert!(
        sample_height.is_finite(),
        "Altura gerada não é finita: {sample_height}"
    );
}

/// Testa se a receita para luas não calibradas (Io, Ganymede, Callisto) causa pânico ao invés de
/// reutilizar silenciosamente a receita de Europa.
#[rstest]
#[case(MoonId::Io)]
#[case(MoonId::Ganymede)]
#[case(MoonId::Callisto)]
#[should_panic(expected = "ainda não calibrada")]
fn uncalibrated_moons_should_panic_instead_of_silently_reusing_europa(#[case] moon_id: MoonId) {
    let profile = minimal_profile_for(moon_id);
    let _ = build_recipe(&profile);
}
