use crate::height::HeightSource;
use crate::params::TerrainParams;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{Mesh, Vec3, default};

/// Gera uma malha de terreno a partir de parâmetros e uma fonte de altura
pub fn build_terrain_mesh(p: TerrainParams, height: &dyn HeightSource) -> Mesh {
    let n = p.res;
    let v_count = (n + 1) as usize;
    let size = p.size;
    let half = size * 0.5;
    let dx = size / n as f32;

    let mut heights = vec![0.0_f32; v_count * v_count];
    for j in 0..v_count {
        for i in 0..v_count {
            let x = -half + i as f32 * dx;
            let z = -half + j as f32 * dx;
            heights[j * v_count + i] = height.height_at(x, z);
        }
    }

    let mut positions = Vec::with_capacity(v_count * v_count);
    let mut uvs = Vec::with_capacity(v_count * v_count);
    for j in 0..v_count {
        for i in 0..v_count {
            let x = -half + i as f32 * dx;
            let z = -half + j as f32 * dx;
            let y = heights[j * v_count + i];
            positions.push([x, y, z]);
            uvs.push([i as f32 / n as f32, j as f32 / n as f32]);
        }
    }

    let mut indices = Vec::with_capacity((n * n * 6) as usize);
    for j in 0..n {
        for i in 0..n {
            let i0 = j * (n + 1) + i;
            let i1 = i0 + 1;
            let i2 = i0 + (n + 1);
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    let mut normals = vec![[0.0, 1.0, 0.0]; v_count * v_count];
    for j in 0..v_count {
        for i in 0..v_count {
            let h_l = heights[j * v_count + i.saturating_sub(1)];
            let h_r = heights[j * v_count + (i + 1).min(v_count - 1)];
            let h_d = heights[j.saturating_sub(1) * v_count + i];
            let h_u = heights[(j + 1).min(v_count - 1) * v_count + i];

            let dh_dx = (h_r - h_l) / (2.0 * dx);
            let dh_dz = (h_u - h_d) / (2.0 * dx);

            let mut n = Vec3::new(-dh_dx, 1.0, -dh_dz).normalize();
            if !n.is_finite() {
                n = Vec3::Y;
            }
            normals[j * v_count + i] = n.to_array();
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.generate_tangents().ok();
    mesh
}
