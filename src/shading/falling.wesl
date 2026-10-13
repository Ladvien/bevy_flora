// Main-pass vertex stage of falling leaves (`falling.rs`, a `FloraMaterial` with `falling` set): each card of a
// burst hangs where it grew until its moment, falls, tumbling and swinging, drifts with the wind,
// settles flat on the ground, lies there a while and shrinks away (`falling_common.wesl`). The
// fragment stage is the tree leaves' own (`fragment.wesl`), so a falling leaf looks like the ones
// still on the tree, season included. The prepass does the same in `falling_prepass.wesl`.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};
import package::shading::common::{leaf_season_tint, FALLEN_LEAF_CLIP};
import package::shading::falling_common::falling_leaf;

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

@if(VERTEX_NORMALS)
    out.world_normal = leaf.normal;

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

    // Out of the crown it is no longer in its tree's shade (`uv1.y`, how much sky it sees).
@if(VERTEX_UVS_B)
    out.uv_b = vec2<f32>(vertex.uv_b.x, mix(vertex.uv_b.y, 1.0, leaf.open));

@if(VERTEX_COLORS)
    out.color = vec4<f32>(vertex.color.rgb * leaf_season_tint(sway, vertex.color), vertex.color.a);

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);

    return out;
}
