//! Construção da malha de um chunk do terreno, com skirt opcional para esconder emendas entre
//! níveis de LOD diferentes.
//!
//! Este é o único núcleo de geração de grade do crate: a malha monolítica de
//! [`crate::mesh::build_terrain_mesh_with_color`] delega para cá.

use crate::height::{ColorSource, HeightSource};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{Mesh, Vec2, Vec3, default};

/// Região quadrada do plano XZ coberta por um chunk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkRegion {
    /// Centro da região no mundo (`x` -> eixo X, `y` -> eixo Z).
    pub center: Vec2,
    /// Lado da região em metros.
    pub size: f32,
}

/// Gera a malha de um chunk.
///
/// - Os vértices ficam em coordenadas locais: X e Z relativos ao centro da região, Y absoluto.
///   A entidade deve ser posicionada em `(center.x, 0, center.y)`.
/// - `quads` é o número de quadrados por lado. Para que níveis diferentes compartilhem vértices
///   na borda, use valores em que um divide o outro.
/// - `skirt_depth` é a profundidade (m) do anel de skirt ao redor da malha. Valor `<= 0.0`
///   desliga o skirt.
/// - `terrain_size` é o lado do terreno inteiro e define as UVs, que vão de 0 a 1 sobre o
///   terreno todo (não sobre o chunk).
/// - As normais vêm de diferenças centrais sobre uma grade com um anel extra de amostras além
///   do chunk, então não há costura de iluminação entre chunks vizinhos do mesmo nível.
/// - Quando `color` é `Some`, escreve também `Mesh::ATTRIBUTE_COLOR` por vértice.
pub fn build_chunk_mesh(
    region: ChunkRegion,
    quads: u32,
    skirt_depth: f32,
    terrain_size: f32,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
) -> Mesh {
    let quads = quads.max(1);
    let vertices_per_side = quads as usize + 1;
    let padded_side = vertices_per_side + 2;
    let step = region.size / quads as f32;
    let half = region.size * 0.5;
    let terrain_half = terrain_size * 0.5;

    // A grade amostrada começa um passo antes da borda, para as normais das bordas usarem
    // diferença central.
    let mut padded_heights = Vec::with_capacity(padded_side * padded_side);
    for row in 0..padded_side {
        for column in 0..padded_side {
            let x = region.center.x - half + (column as f32 - 1.0) * step;
            let z = region.center.y - half + (row as f32 - 1.0) * step;
            padded_heights.push(height.height_at(x, z));
        }
    }
    let padded_height = |column: usize, row: usize| padded_heights[row * padded_side + column];

    let skirt_vertex_count = if skirt_depth > 0.0 { 4 * vertices_per_side } else { 0 };
    let vertex_count = vertices_per_side * vertices_per_side + skirt_vertex_count;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_count);
    let mut colors: Option<Vec<[f32; 4]>> = color.map(|_| Vec::with_capacity(vertex_count));

    for row in 0..vertices_per_side {
        for column in 0..vertices_per_side {
            let local_x = -half + column as f32 * step;
            let local_z = -half + row as f32 * step;
            let world_x = region.center.x + local_x;
            let world_z = region.center.y + local_z;

            positions.push([local_x, padded_height(column + 1, row + 1), local_z]);
            uvs.push([
                (world_x + terrain_half) / terrain_size,
                (world_z + terrain_half) / terrain_size,
            ]);

            let dh_dx =
                (padded_height(column + 2, row + 1) - padded_height(column, row + 1)) / (2.0 * step);
            let dh_dz =
                (padded_height(column + 1, row + 2) - padded_height(column + 1, row)) / (2.0 * step);
            let mut normal = Vec3::new(-dh_dx, 1.0, -dh_dz).normalize();
            if !normal.is_finite() {
                normal = Vec3::Y;
            }
            normals.push(normal.to_array());

            if let (Some(color), Some(colors)) = (color, colors.as_mut()) {
                colors.push(color.color_at(world_x, world_z));
            }
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((quads * quads * 6) as usize);
    let top_index = |column: u32, row: u32| row * (quads + 1) + column;
    for row in 0..quads {
        for column in 0..quads {
            let i0 = top_index(column, row);
            let i1 = i0 + 1;
            let i2 = i0 + (quads + 1);
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    if skirt_depth > 0.0 {
        // Cada borda é percorrida no sentido em que `direção x para-baixo` aponta para fora
        // do chunk, que é o que dá a face voltada para fora com o winding anti-horário.
        let last = quads;
        let edges: [Vec<u32>; 4] = [
            (0..=last).map(|column| top_index(column, 0)).collect(),
            (0..=last).rev().map(|column| top_index(column, last)).collect(),
            (0..=last).map(|row| top_index(last, row)).collect(),
            (0..=last).rev().map(|row| top_index(0, row)).collect(),
        ];

        for edge in &edges {
            let skirt_base = positions.len() as u32;
            for &top in edge {
                let top = top as usize;
                let [x, y, z] = positions[top];
                positions.push([x, y - skirt_depth, z]);
                normals.push(normals[top]);
                uvs.push(uvs[top]);
                if let Some(colors) = colors.as_mut() {
                    colors.push(colors[top]);
                }
            }
            for segment in 0..quads {
                let top_a = edge[segment as usize];
                let top_b = edge[segment as usize + 1];
                let bottom_a = skirt_base + segment;
                let bottom_b = bottom_a + 1;
                indices.extend_from_slice(&[top_a, top_b, bottom_a, top_b, bottom_b, bottom_a]);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    if let Some(colors) = colors {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    mesh.insert_indices(Indices::U32(indices));
    mesh.generate_tangents().ok();
    mesh
}
