// What a tree is drawn with to bake its impostor: unlit, one of two pictures. The Rust side is
// `impostor/bake.rs` and `impostor/material.rs`.
//
// Colour pass: the material's colour times the mesh's vertex colours, alpha 1 (the clear colour
// is transparent, so alpha is the tree's coverage).
// Surface pass: the mesh's normal in the tree's own frame, octahedral-encoded, in rg; the share of
// the sky the leaf sees (the mesh's second UV set, or 1) in b; how much light the surface lets
// through in a. `impostor.wesl` lights these at runtime.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};
import package::shading::common::oct_encode;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
// x: 0 colour pass, 1 surface pass; y: translucency; z: visibility without a second UV set.
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> params: vec4<f32>;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    out.position = position_world_to_clip(out.world_position.xyz);
    // The surface picture keeps normals in the tree's own frame, so the copy's turn does not
    // touch them.
@if(VERTEX_NORMALS)
    out.world_normal = vertex.normal;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_COLORS) {
    out.color = vertex.color;
    // Flowers (colour alpha 0) are left out of the picture, which is summer's; the impostor
    // paints its blossom over it in season.
    if vertex.color.a < 0.5 {
        out.position = vec4<f32>(2.0, 2.0, 0.5, 1.0);
    }
}
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    var tint = vec3<f32>(1.0);
@if(VERTEX_COLORS)
    tint = in.color.rgb;
    var visibility = params.z;
@if(VERTEX_UVS_B)
    visibility = in.uv_b.y;

    if params.x < 0.5 {
        out.color = vec4<f32>(color.rgb * tint, 1.0);
    } else {
        out.color = vec4<f32>(oct_encode(normalize(in.world_normal)) * 0.5 + 0.5, visibility, params.y);
    }
    return out;
}
