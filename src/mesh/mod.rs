//! CPU-side mesh buffers and the meshers that fill them.

pub mod branch;
pub mod rings;
pub mod smoothing;
pub mod trunk;

use bevy::asset::RenderAssetUsages;
use bevy::math::{Vec2, Vec3, Vec4};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

/// Triangle-list geometry with positions, normals and one UV set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// Vertex positions, in metres.
    pub positions: Vec<Vec3>,
    /// Unit vertex normals, one per position.
    pub normals: Vec<Vec3>,
    /// Texture coordinates, one per position.
    pub uvs: Vec<Vec2>,
    /// Linear RGBA vertex colours, one per position, or empty for no colour attribute.
    pub colors: Vec<Vec4>,
    /// Triangle-list indices into the vertex arrays; length is a multiple of 3.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Append `other`, offsetting its indices. If either side carries vertex
    /// colours, the other side's vertices are padded with opaque white.
    pub fn extend(&mut self, other: &MeshData) {
        let offset = self.positions.len() as u32;
        if !self.colors.is_empty() || !other.colors.is_empty() {
            self.colors.resize(offset as usize, Vec4::ONE);
            if other.colors.is_empty() {
                self.colors
                    .resize(offset as usize + other.positions.len(), Vec4::ONE);
            } else {
                self.colors.extend_from_slice(&other.colors);
            }
        }
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.indices.extend(other.indices.iter().map(|i| i + offset));
    }

    /// True when there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Convert into a Bevy [`Mesh`] (triangle list, `u32` indices).
    pub fn into_bevy_mesh(self) -> Mesh {
        let positions: Vec<[f32; 3]> = self.positions.iter().map(Vec3::to_array).collect();
        let normals: Vec<[f32; 3]> = self.normals.iter().map(Vec3::to_array).collect();
        let uvs: Vec<[f32; 2]> = self.uvs.iter().map(Vec2::to_array).collect();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(self.indices));
        if !self.colors.is_empty() {
            let colors: Vec<[f32; 4]> = self.colors.iter().map(Vec4::to_array).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
        mesh
    }
}
