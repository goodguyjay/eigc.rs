use bevy::mesh::VertexAttributeValues;
use bevy::prelude::{Mesh, Vec2};
use eigc_terrain::height::{ColorSource, HeightSource};
use eigc_terrain::mesh::{build_terrain_mesh, build_terrain_mesh_with_color};
use eigc_terrain::params::TerrainParams;

/// Uma implementação de HeightSource que sempre retorna altura zero.
struct FlatHeight;

/// Implementação de HeightSource para FlatHeight.
impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Uma implementação de ColorSource que pinta metades opostas do eixo X com cores diferentes,
/// para verificar que a malha recebe cor não-uniforme por vértice.
struct SplitColor;

impl ColorSource for SplitColor {
    fn color_at(&self, x: f32, _z: f32) -> [f32; 4] {
        if x < 0.0 {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        }
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

/// Teste para verificar que build_terrain_mesh (sem cor) não escreve ATTRIBUTE_COLOR na malha.
#[test]
fn mesh_without_color_source_has_no_color_attribute() {
    let params = TerrainParams {
        size: 100.0,
        res: 4,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0,
    };

    let mesh = build_terrain_mesh(params, &FlatHeight);

    assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
}

/// Teste para verificar que, quando uma ColorSource é fornecida, a malha ganha ATTRIBUTE_COLOR
/// com valores não-uniformes correspondentes à cor amostrada em cada vértice.
#[test]
fn mesh_with_color_source_includes_non_uniform_vertex_color_attribute() {
    let params = TerrainParams {
        size: 100.0,
        res: 4,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0,
    };

    let mesh = build_terrain_mesh_with_color(params, &FlatHeight, Some(&SplitColor));

    let colors = match mesh
        .attribute(Mesh::ATTRIBUTE_COLOR)
        .expect("malha deveria ter ATTRIBUTE_COLOR quando uma ColorSource é fornecida")
    {
        VertexAttributeValues::Float32x4(colors) => colors,
        other => panic!("formato inesperado para ATTRIBUTE_COLOR: {other:?}"),
    };

    assert!(colors.iter().any(|c| c[0] > 0.5), "deveria ter vértices vermelhos (x < 0)");
    assert!(colors.iter().any(|c| c[2] > 0.5), "deveria ter vértices azuis (x >= 0)");
}
