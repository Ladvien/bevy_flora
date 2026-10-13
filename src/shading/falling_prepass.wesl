// Prepass vertex stage of falling leaves (depth, normal, motion-vector prepasses and shadow maps):
// the same fall as `falling.wesl`, so their shadows fall with them. The previous position is the
// current one, as for the trees' wind.
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{mesh_functions, view_transformations::position_world_to_clip},
};
import bevy_render::globals::Globals;
import package::shading::common::FALLEN_LEAF_CLIP;
import package::shading::falling_common::falling_leaf;

// The prepass's view bind group (view, globals, previous view); `bevy_pbr` has no WESL
// declaration of it.
@group(0) @binding(1) var<uniform> globals: Globals;

@vertex
fn vertex(
    vertex_no_morph: Vertex,
    @location(11) sway: vec4<f32>,
    @location(12) anchor: vec4<f32>,
    @location(13) path: vec4<f32>,
    @location(14) face: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
    var normal = vec3<f32>(0.0, 1.0, 0.0);
@if(VERTEX_NORMALS)
    normal = vertex.normal;
    let leaf = falling_leaf(world_from_local, vertex.position, normal, anchor, path, face, globals.time);
    if !leaf.shown {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
    out.world_position = vec4<f32>(leaf.position, 1.0);
    out.position = position_world_to_clip(leaf.position);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    out.world_normal = leaf.normal;

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);

    return out;
}
