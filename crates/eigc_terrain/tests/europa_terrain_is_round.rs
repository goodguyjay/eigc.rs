//! Testes de integração do recorte redondo de Europa: a altura desce até um piso plano na borda
//! do círculo inscrito e os chunks totalmente fora do círculo não são criados.

use bevy::MinimalPlugins;
use bevy::prelude::{
    App, AssetApp, AssetPlugin, Assets, Commands, Handle, Mesh, ResMut, StandardMaterial, Startup,
    Vec2,
};
use eigc_moons::profile::{
    MoonId, MoonProfile, SkyCalibration, SkyGeometry, TerrainCalibration,
};
use eigc_terrain::chunks::{TerrainChunk, spawn_terrain_chunks};
use eigc_terrain::recipe::{build_recipe, playable_radius_m};

/// Cria um MoonProfile mínimo para teste, com valores arbitrários, exceto pelo moon_id.
fn europa_profile() -> MoonProfile {
    MoonProfile {
        moon_id: MoonId::Europa,
        display_name: "Lua teste".to_string(),
        jupiter_angular_diameter_deg: 10.0,
        terrain: TerrainCalibration {
            seed: 99,
            base_frequency: 0.001,
            feature_direction: [1.0, 0.0],
            warp_amplitude_meters: 20.0,
            perceptual_roughness: 0.5,
            reflectance: 0.3,
        },
        terrain_base_color: [1.0, 1.0, 1.0, 1.0],
        terrain_steep_color: [0.3, 0.2, 0.1, 1.0],
        walkable: true,
        sky: SkyCalibration {
            geometry: SkyGeometry::Fixed {
                base_jupiter_dir: [0.0, 0.0, -1.0],
                sun_elevation_deg: 1.0,
            },
            planet_shine_max: 0.01,
            jupiter_ang_radius: 0.10384709,
            orbital_period_seconds: 3.551181 * 86_400.0,
            eclipse_soft_deg: 1.0,
            jupiter_libration_lat_deg: 2.0,
            jupiter_libration_lon_deg: 2.0,
        },
    }
}

/// Pontos de uma grade quadrada de `step_m` metros que cobre o recorte de `half_size_m` de meia
/// largura.
fn grid_points(half_size_m: f32, step_m: f32) -> Vec<Vec2> {
    let steps = (2.0 * half_size_m / step_m) as i32;
    (0..=steps)
        .flat_map(|i| {
            (0..=steps).map(move |j| {
                Vec2::new(
                    -half_size_m + i as f32 * step_m,
                    -half_size_m + j as f32 * step_m,
                )
            })
        })
        .collect()
}

/// Testa que a altura é a mesma, o piso, em qualquer ponto a partir do raio do círculo inscrito,
/// inclusive nos cantos do recorte quadrado.
#[test]
fn europa_height_beyond_the_disc_edge_is_the_flat_floor() {
    let recipe = build_recipe(&europa_profile());
    let radius = recipe.params.disc_radius_m();
    let floor = recipe.height.height_at(radius, 0.0);

    let beyond_the_edge = [
        (radius, 0.0),
        (0.0, -radius),
        (-radius - 1.0, 40.0),
        (radius * 0.8, radius * 0.8),
        (-radius, -radius),
        (radius * 2.0, 700.0),
    ];
    for (x, z) in beyond_the_edge {
        assert_eq!(
            recipe.height.height_at(x, z),
            floor,
            "ponto ({x}, {z}) além da borda deveria estar no piso"
        );
    }
}

/// Testa que o piso fica abaixo de todo o relevo antes da rampa (até 9 km do centro), para a rampa
/// só descer. A rampa em si chega perto do piso por definição, então não entra na amostra.
#[test]
fn europa_edge_floor_is_below_the_whole_relief() {
    let recipe = build_recipe(&europa_profile());
    let radius = recipe.params.disc_radius_m();
    let floor = recipe.height.height_at(radius, 0.0);

    const BEFORE_THE_RAMP_M: f32 = 9_000.0;
    let mut lowest = f32::MAX;
    for point in grid_points(BEFORE_THE_RAMP_M, 250.0) {
        if point.length() < BEFORE_THE_RAMP_M {
            lowest = lowest.min(recipe.height.height_at(point.x, point.y));
        }
    }
    assert!(
        floor < lowest - 10.0,
        "piso {floor} deveria ficar bem abaixo do ponto mais baixo do relevo {lowest}"
    );
}

/// Testa que o raio jogável fica dentro do círculo e que, nele, a altura já está acima do piso.
#[test]
fn europa_playable_radius_is_inside_the_disc_and_above_the_floor() {
    let recipe = build_recipe(&europa_profile());
    let radius = recipe.params.disc_radius_m();
    let floor = recipe.height.height_at(radius, 0.0);

    assert!(playable_radius_m() < radius);
    let at_playable = recipe.height.height_at(playable_radius_m(), 0.0);
    assert!(
        at_playable > floor,
        "no raio jogável a altura {at_playable} deveria estar acima do piso {floor}"
    );
}

/// Testa que a receita de Europa cria menos de 4096 chunks, que nenhum canto da grade é criado,
/// que o chunk do meio existe e que os chunks cobrem o círculo inteiro.
#[test]
fn europa_spawns_only_the_chunks_that_touch_the_disc() {
    let recipe = build_recipe(&europa_profile());
    let chunks_per_side = recipe.lod.chunks_per_side;
    let size = recipe.params.size;
    let radius = recipe.params.disc_radius_m();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .add_systems(
            Startup,
            move |mut commands: Commands,
                  mut meshes: ResMut<Assets<Mesh>>,
                  mut materials: ResMut<Assets<StandardMaterial>>| {
                let material: Handle<StandardMaterial> = materials.add(StandardMaterial::default());
                spawn_terrain_chunks(
                    &mut commands,
                    &mut meshes,
                    material,
                    &recipe.params,
                    &recipe.lod,
                    recipe.height.as_ref(),
                    None,
                );
            },
        );
    app.update();

    let coords: Vec<(u32, u32)> = app
        .world_mut()
        .query::<&TerrainChunk>()
        .iter(app.world())
        .map(|chunk| (chunk.coord.ix, chunk.coord.iz))
        .collect();

    let full_grid = (chunks_per_side * chunks_per_side) as usize;
    assert!(
        coords.len() < full_grid,
        "deveria haver menos chunks que a grade cheia ({full_grid}), há {}",
        coords.len()
    );

    let last = chunks_per_side - 1;
    for corner in [(0, 0), (last, 0), (0, last), (last, last)] {
        assert!(
            !coords.contains(&corner),
            "canto {corner:?} não deveria existir"
        );
    }
    let middle = chunks_per_side / 2;
    assert!(coords.contains(&(middle, middle)));

    let chunk_area = (size / chunks_per_side as f32).powi(2);
    let covered_area = coords.len() as f32 * chunk_area;
    let disc_area = std::f32::consts::PI * radius * radius;
    assert!(
        covered_area >= disc_area,
        "os chunks cobrem {covered_area} m2, menos que o disco de {disc_area} m2"
    );
}
