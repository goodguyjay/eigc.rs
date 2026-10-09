//! Testes de integração para recipe::build_recipe. Cobre as quatro luas do enum MoonId

use eigc_moons::profile::{MoonId, MoonProfile, SkyCalibration, TerrainCalibration};
use eigc_terrain::recipe::build_recipe;
use rstest::rstest;

/// Cria um MoonProfile mínimo para teste, com valores arbitrários, exceto pelo moon_id.
/// Não representa valores reais da aplicação.
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
        terrain_valley_color: [0.3, 0.2, 0.1, 1.0],
        walkable: true,
        sky: SkyCalibration {
            base_jupiter_dir: [0.0, 0.0, -1.0],
            base_sun_dir: [0.0, 1.0, 0.0],
            planet_shine_max: 0.01,
            jupiter_ang_radius: 0.10384709,
            orbital_period_seconds: 3.551181 * 86_400.0,
            eclipse_soft_deg: 1.0,
            jupiter_libration_lat_deg: 2.0,
            jupiter_libration_lon_deg: 2.0,
            sun_elevation_deg: 1.0,
        },
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
        recipe.material_properties.perceptual_roughness, profile.terrain.perceptual_roughness,
        "perceptual_roughness não corresponde ao perfil"
    );

    let sample_height = recipe.height.height_at(100.0, 200.0);
    assert!(
        sample_height.is_finite(),
        "Altura gerada não é finita: {sample_height}"
    );

    assert!(
        recipe.color.is_some(),
        "Europa deveria produzir uma fonte de cor por vértice (inclinação do DTM)"
    );
}

/// Testa se a configuração de LOD de Europa é coerente: níveis do mais fino ao mais grosso,
/// distâncias crescentes e uma grade de chunks que cobre exatamente o terreno.
#[test]
fn europa_recipe_produces_a_consistent_lod_config() {
    let profile = minimal_profile_for(MoonId::Europa);
    let recipe = build_recipe(&profile);
    let lod = &recipe.lod;

    assert!(lod.chunks_per_side > 0);
    assert!(
        lod.quads_per_chunk.windows(2).all(|pair| pair[0] > pair[1]),
        "quads por chunk deveriam decrescer do nível 0 para o mais grosso: {:?}",
        lod.quads_per_chunk
    );
    assert!(
        lod.quads_per_chunk
            .windows(2)
            .all(|pair| pair[0] % pair[1] == 0),
        "cada nível deveria dividir o anterior, para as bordas coincidirem: {:?}",
        lod.quads_per_chunk
    );
    assert!(
        lod.max_screen_error_px > 0.0,
        "a tolerância de erro em pixels deveria ser positiva: {}",
        lod.max_screen_error_px
    );
    assert!(lod.min_error_fraction >= 0.0);
    assert!(lod.hysteresis_fraction >= 0.0 && lod.hysteresis_fraction < 1.0);
    assert!(lod.in_flight_vertex_budget > 0);
    assert!(lod.max_error_tasks_in_flight > 0);

    let chunk_size = recipe.params.size / lod.chunks_per_side as f32;
    assert!(
        (chunk_size * lod.chunks_per_side as f32 - recipe.params.size).abs() < 1e-3,
        "a grade de chunks deveria cobrir o terreno inteiro"
    );
}

/// Testa se a origem (onde câmera/jogador aparecem) e uma vizinhança ao redor dela ficam planas.
#[test]
fn europa_recipe_keeps_spawn_clearing_flat_around_origin() {
    let profile = minimal_profile_for(MoonId::Europa);
    let recipe = build_recipe(&profile);

    let origin_height = recipe.height.height_at(0.0, 0.0);
    for (x, z) in [(0.0, 0.0), (100.0, 0.0), (0.0, -140.0), (-90.0, 90.0)] {
        let height = recipe.height.height_at(x, z);
        assert_eq!(
            height, origin_height,
            "ponto ({x}, {z}) dentro da clareira deveria ter a mesma altura plana da origem"
        );
    }
}

/// Testa se o relevo de Europa vem do DTM real: fora da clareira, varia em centenas de metros
/// (o ruído de fundo sozinho variava ~12 m) e fica recentrado em torno de y=0, não nas
/// elevações absolutas do DTM (-514 a -64 m).
#[test]
fn europa_recipe_relief_follows_the_real_dtm_recentered_around_zero() {
    let profile = minimal_profile_for(MoonId::Europa);
    let recipe = build_recipe(&profile);

    let half_size = recipe.params.size * 0.5;
    let steps = 40;
    let mut lowest = f32::MAX;
    let mut highest = f32::MIN;
    for i in 0..=steps {
        for j in 0..=steps {
            let x = -half_size + recipe.params.size * i as f32 / steps as f32;
            let z = -half_size + recipe.params.size * j as f32 / steps as f32;
            let height = recipe.height.height_at(x, z);
            assert!(height.is_finite(), "altura não finita em ({x}, {z})");
            lowest = lowest.min(height);
            highest = highest.max(height);
        }
    }

    assert!(
        highest - lowest > 300.0,
        "o relevo deveria vir do DTM e variar centenas de metros, variou {}",
        highest - lowest
    );
    assert!(
        lowest < 0.0 && highest > 0.0,
        "o relevo deveria ficar recentrado em torno de zero: de {lowest} a {highest}"
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
