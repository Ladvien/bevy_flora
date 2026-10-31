// Prepass vertex stage of a tree drawn with `FloraMaterial` (depth, normal, motion-vector
// prepasses and shadow maps): Bevy's own, with the same flutter and bend as `vertex.wesl`.
// Wind does not feed motion vectors: the previous position is the current one.
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{mesh_functions, view_transformations::position_world_to_clip},
};
import bevy_render::globals::Globals;
import package::shading::common::{flutter_offset, shake_at, wind_bend};

@if(FLORA_LEAF)
import package::shading::common::{leaf_has_fallen, leaf_hidden, leaf_defoliated, tags_read, FALLEN_LEAF_CLIP};

@if(FLORA_FIRE && FLORA_LEAF)
import package::shading::common::{fire_at, leaf_burnt};

@if(FLORA_PUSH)
import package::shading::common::{push_offset, wood_flex};

// The prepass's view bind group (view, globals, previous view); `bevy_pbr` has no WESL
// declaration of it.
@group(0) @binding(1) var<uniform> globals: Globals;

@vertex
fn vertex(
    vertex_no_morph: Vertex,
@if(FLORA_LEAF)
    @location(11) sway: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    // Hidden leaves and flowers leave early, as in `vertex.wesl`, and so do the leaves a sick or
    // dying tree has lost (`TreeCondition`): the gaps in its crown let the light through.
@if(FLORA_LEAF)
    var tag = 0u;
@if(FLORA_LEAF)
    if tags_read() {
        tag = mesh_functions::get_tag(vertex_no_morph.instance_index);
    }
@if(FLORA_LEAF && VERTEX_COLORS) {
    if leaf_hidden(sway, vertex.color) || leaf_defoliated(sway, vertex.color.a, tag) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
@if(FLORA_LEAF && !VERTEX_COLORS) {
    if leaf_has_fallen(sway) || leaf_defoliated(sway, 1.0, tag) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let origin = world_from_local[3].xyz;
    // A leaf the fire has burnt casts no shadow either, as in `vertex.wesl`.
@if(FLORA_FIRE && FLORA_LEAF) {
    if leaf_burnt(sway, fire_at(origin)) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
    let shake = shake_at(origin, globals.time);
    var local = vertex.position;
@if(FLORA_LEAF)
    local = local + flutter_offset(sway, origin, globals.time, shake);

    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.world_position = vec4<f32>(
        wind_bend(world.xyz, origin, globals.time, shake),
        1.0
    );
    // Pushed aside by a body moving through, as in `vertex.wesl`.
@if(FLORA_PUSH) {
    var flex = 0.0;
@if(FLORA_LEAF)
    flex = 1.0;
@if(!FLORA_LEAF && VERTEX_UVS_B)
    flex = wood_flex(vertex.uv_b.x * length(world_from_local[1].xyz));
    let pushed = out.world_position.xyz + push_offset(out.world_position.xyz, origin, flex);
    out.world_position = vec4<f32>(pushed, 1.0);
}
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

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
