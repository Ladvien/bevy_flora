// Prepass vertex stage of a growing tree (depth, normal, motion-vector prepasses
// and shadow maps): Bevy's own, with the position taken from the growth state.
// Growth does not feed motion vectors: the previous position is the current one.
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{mesh_functions, view_transformations::position_world_to_clip},
};
import package::gpu::growth_common::{GrowthAttrs, grown_position};

@vertex
fn vertex(vertex_no_morph: Vertex, growth_attrs: GrowthAttrs) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(grown_position(growth_attrs), 1.0)
    );
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );

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
