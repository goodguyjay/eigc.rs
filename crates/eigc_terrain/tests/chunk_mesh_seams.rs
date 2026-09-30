use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::{Mesh, Vec2, Vec3};
use eigc_terrain::chunk_mesh::{ChunkRegion, build_chunk_mesh};
use eigc_terrain::height::{ColorSource, HeightSource};

/// Relevo suave e não-linear, para que diferenças de amostragem entre chunks apareçam.
struct WavyHeight;

impl HeightSource for WavyHeight {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        (x * 0.031).sin() * 10.0 + (z * 0.017).cos() * 5.0
    }
}

/// Altura zero em todo lugar.
struct FlatHeight;

impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Cor que depende da posição, para conferir que o skirt copia a cor do vértice de borda.
struct GradientColor;

impl ColorSource for GradientColor {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        [x * 0.001, z * 0.001, 0.5, 1.0]
    }
}

const TERRAIN_SIZE: f32 = 400.0;
const CHUNK_SIZE: f32 = 200.0;

fn region(center_x: f32, center_z: f32) -> ChunkRegion {
    ChunkRegion {
        center: Vec2::new(center_x, center_z),
        size: CHUNK_SIZE,
    }
}

fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        .expect("malha sem posições")
        .as_float3()
        .expect("posições em formato inesperado")
}

fn normals(mesh: &Mesh) -> &[[f32; 3]] {
    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        .expect("malha sem normais")
        .as_float3()
        .expect("normais em formato inesperado")
}

fn indices(mesh: &Mesh) -> &[u32] {
    match mesh.indices().expect("malha sem índices") {
        Indices::U32(indices) => indices,
        Indices::U16(_) => panic!("índices deveriam ser U32"),
    }
}

/// Índice do vértice de grade (sem skirt) na coluna e linha dadas.
fn grid_index(quads: u32, column: u32, row: u32) -> usize {
    (row * (quads + 1) + column) as usize
}

/// Posição do vértice em coordenadas de mundo, somando o centro da região.
fn world_position(vertex: [f32; 3], center: Vec2) -> Vec3 {
    Vec3::new(vertex[0] + center.x, vertex[1], vertex[2] + center.y)
}

/// Sem skirt, a malha tem exatamente a grade `(q+1)^2` e `q^2 * 6` índices.
#[test]
fn chunk_without_skirt_has_only_grid_vertices_and_indices() {
    let mesh = build_chunk_mesh(region(0.0, 0.0), 8, 0.0, TERRAIN_SIZE, &FlatHeight, None);

    assert_eq!(positions(&mesh).len(), 9 * 9);
    assert_eq!(indices(&mesh).len(), 8 * 8 * 6);
}

/// Com skirt, entram 4 bordas de `q+1` vértices duplicados e `4 * q * 6` índices.
#[test]
fn chunk_with_skirt_adds_four_edges_of_duplicated_vertices() {
    let mesh = build_chunk_mesh(region(0.0, 0.0), 8, 5.0, TERRAIN_SIZE, &FlatHeight, None);

    assert_eq!(positions(&mesh).len(), 9 * 9 + 4 * 9);
    assert_eq!(indices(&mesh).len(), 8 * 8 * 6 + 4 * 8 * 6);
}

/// Chunks vizinhos do mesmo nível têm posições e normais idênticas na borda compartilhada.
#[test]
fn adjacent_chunks_at_same_level_share_edge_positions_and_normals() {
    let left_center = Vec2::new(-100.0, 0.0);
    let right_center = Vec2::new(100.0, 0.0);
    let quads = 8;
    let left = build_chunk_mesh(region(left_center.x, left_center.y), quads, 0.0, TERRAIN_SIZE, &WavyHeight, None);
    let right = build_chunk_mesh(region(right_center.x, right_center.y), quads, 0.0, TERRAIN_SIZE, &WavyHeight, None);

    for row in 0..=quads {
        let left_vertex = grid_index(quads, quads, row);
        let right_vertex = grid_index(quads, 0, row);

        let left_world = world_position(positions(&left)[left_vertex], left_center);
        let right_world = world_position(positions(&right)[right_vertex], right_center);
        assert!(
            left_world.distance(right_world) < 1e-3,
            "posição diverge na linha {row}: {left_world:?} vs {right_world:?}"
        );

        let left_normal = Vec3::from(normals(&left)[left_vertex]);
        let right_normal = Vec3::from(normals(&right)[right_vertex]);
        assert!(
            left_normal.distance(right_normal) < 1e-3,
            "normal diverge na linha {row}: {left_normal:?} vs {right_normal:?}"
        );
    }
}

/// Os vértices de borda do nível grosso coincidem com vértices do nível fino (nenhuma fresta
/// além da que o skirt cobre).
#[test]
fn coarse_edge_vertices_coincide_with_a_subset_of_fine_edge_vertices() {
    let center = Vec2::new(-100.0, 0.0);
    let coarse_quads = 4;
    let fine_quads = 8;
    let coarse = build_chunk_mesh(region(center.x, center.y), coarse_quads, 0.0, TERRAIN_SIZE, &WavyHeight, None);
    let fine = build_chunk_mesh(region(center.x, center.y), fine_quads, 0.0, TERRAIN_SIZE, &WavyHeight, None);

    let ratio = fine_quads / coarse_quads;
    for row in 0..=coarse_quads {
        let coarse_vertex = positions(&coarse)[grid_index(coarse_quads, coarse_quads, row)];
        let fine_vertex = positions(&fine)[grid_index(fine_quads, fine_quads, row * ratio)];
        assert!(
            Vec3::from(coarse_vertex).distance(Vec3::from(fine_vertex)) < 1e-3,
            "vértice de borda do nível grosso não existe no fino (linha {row})"
        );
    }
}

/// Cada vértice de skirt fica exatamente `skirt_depth` abaixo do vértice de borda de origem.
#[test]
fn skirt_vertices_sit_directly_below_the_edge_vertices() {
    let quads = 4;
    let depth = 7.0;
    let mesh = build_chunk_mesh(region(0.0, 0.0), quads, depth, TERRAIN_SIZE, &WavyHeight, None);
    let positions = positions(&mesh);

    // A primeira borda do skirt percorre a linha 0 com a coluna crescendo.
    let skirt_base = ((quads + 1) * (quads + 1)) as usize;
    for column in 0..=quads {
        let top = positions[grid_index(quads, column, 0)];
        let bottom = positions[skirt_base + column as usize];
        assert_eq!(bottom[0], top[0]);
        assert_eq!(bottom[2], top[2]);
        assert!((top[1] - bottom[1] - depth).abs() < 1e-4);
    }
}

/// Em terreno plano os triângulos da grade olham para cima e os do skirt olham para fora.
#[test]
fn grid_triangles_face_up_and_skirt_triangles_face_outward() {
    let quads = 4;
    let mesh = build_chunk_mesh(region(0.0, 0.0), quads, 5.0, TERRAIN_SIZE, &FlatHeight, None);
    let positions = positions(&mesh);
    let indices = indices(&mesh);
    let grid_index_count = (quads * quads * 6) as usize;

    for (triangle_number, triangle) in indices.chunks_exact(3).enumerate() {
        let a = Vec3::from(positions[triangle[0] as usize]);
        let b = Vec3::from(positions[triangle[1] as usize]);
        let c = Vec3::from(positions[triangle[2] as usize]);
        let face_normal = (b - a).cross(c - a);
        assert!(face_normal.length() > 0.0, "triângulo degenerado {triangle_number}");

        if triangle_number * 3 < grid_index_count {
            assert!(face_normal.y > 0.0, "triângulo de grade {triangle_number} olha para baixo");
        } else {
            let centroid = (a + b + c) / 3.0;
            let outward = Vec3::new(centroid.x, 0.0, centroid.z);
            assert!(
                face_normal.dot(outward) > 0.0,
                "triângulo de skirt {triangle_number} olha para dentro"
            );
        }
    }
}

/// Em terreno plano todas as normais, inclusive as do skirt, apontam para cima.
#[test]
fn flat_chunk_has_upward_normals_including_skirt() {
    let mesh = build_chunk_mesh(region(0.0, 0.0), 4, 5.0, TERRAIN_SIZE, &FlatHeight, None);

    for normal in normals(&mesh) {
        assert!((normal[1] - 1.0).abs() < 1e-5, "normal não é vertical: {normal:?}");
    }
}

/// As UVs são globais: um chunk no canto do terreno cobre só um quarto do intervalo 0..1.
#[test]
fn uvs_span_the_whole_terrain_not_just_the_chunk() {
    let mesh = build_chunk_mesh(region(-100.0, -100.0), 4, 0.0, TERRAIN_SIZE, &FlatHeight, None);
    let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).expect("malha sem UVs") {
        VertexAttributeValues::Float32x2(uvs) => uvs,
        other => panic!("formato inesperado para UV: {other:?}"),
    };

    assert!((uvs[0][0] - 0.0).abs() < 1e-5 && (uvs[0][1] - 0.0).abs() < 1e-5);
    let last = uvs[uvs.len() - 1];
    assert!((last[0] - 0.5).abs() < 1e-5 && (last[1] - 0.5).abs() < 1e-5);
}

/// A cor por vértice tem um valor por vértice, inclusive nos de skirt, e o skirt copia a cor
/// do vértice de borda.
#[test]
fn skirt_vertices_copy_the_color_of_their_edge_vertex() {
    let quads = 4;
    let mesh = build_chunk_mesh(region(0.0, 0.0), quads, 5.0, TERRAIN_SIZE, &FlatHeight, Some(&GradientColor));
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("malha sem cor") {
        VertexAttributeValues::Float32x4(colors) => colors,
        other => panic!("formato inesperado para cor: {other:?}"),
    };

    assert_eq!(colors.len(), positions(&mesh).len());
    let skirt_base = ((quads + 1) * (quads + 1)) as usize;
    for column in 0..=quads {
        assert_eq!(colors[skirt_base + column as usize], colors[grid_index(quads, column, 0)]);
    }
}
