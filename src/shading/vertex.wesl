// Main-pass vertex stage of a tree drawn with `FloraMaterial`: Bevy's own, with the leaves
// fluttering and the whole tree bending in the wind. Fragment: `fragment.wesl`. The prepass
// (depth, shadow maps) does the same in `prepass.wesl`, or shadows would come loose.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};
import package::shading::common::{flutter_offset, wind_bend};

@if(FLORA_LEAF)
import package::shading::common::{leaf_has_fallen, leaf_autumn_tint, FALLEN_LEAF_CLIP};

@vertex
fn vertex(
    vertex_no_morph: Vertex,
@if(FLORA_LEAF)
    @location(11) sway: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let origin = world_from_local[3].xyz;
    var local = vertex.position;
@if(FLORA_LEAF)
    local = local + flutter_offset(sway, origin, globals.time);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );

    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.world_position = vec4<f32>(
        wind_bend(world.xyz, local.y, origin, length(world_from_local[1].xyz), globals.time),
        1.0
    );
    out.position = position_world_to_clip(out.world_position.xyz);
@if(FLORA_LEAF) {
    if leaf_has_fallen(sway) {
        out.position = FALLEN_LEAF_CLIP;
    }
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

    // A deciduous tree's leaves turn their autumn colour, each at its own moment.
@if(VERTEX_COLORS && !FLORA_LEAF)
    out.color = vertex.color;
@if(VERTEX_COLORS && FLORA_LEAF)
    out.color = vec4<f32>(vertex.color.rgb * leaf_autumn_tint(sway), vertex.color.a);

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);

    return out;
}
