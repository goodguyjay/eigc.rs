use bevy::prelude::{Mesh, Vec2};
use eigc_terrain::height::HeightSource;
use eigc_terrain::mesh::build_terrain_mesh;
use eigc_terrain::params::TerrainParams;

/// Uma implementação de HeightSource que sempre retorna altura zero.
struct FlatHeight;

/// Implementação de HeightSource para FlatHeight.
impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Teste para verificar se a função de altura plana produz uma malha plana com normais voltadas para cima.
#[test]
fn flat_height_produces_flat_mesh_with_upward_normals() {
    let params = TerrainParams {
        size: 100.0,
        res: 4,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0
    };

    let mesh = build_terrain_mesh(params, &FlatHeight);

    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();

    // altura zero em todo lugar, y deve ser 0.0 em cada vértice
    assert!(positions.iter().all(|p| p[1] == 0.0));
    
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    
    // superfície plana, normal deveria apontar reto pra cima em todo vértice
    for n in normals {
        assert!((n[1] - 1.0).abs() < 1e-5, "normal não é vertical: {:?}", n);
    }
}
