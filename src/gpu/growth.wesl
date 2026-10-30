// Main-pass vertex stage of a growing tree: Bevy's own vertex stage, with the
// position taken from the growth state, then the flutter of the leaves and the bend of
// the tree in the wind exactly as `shading/vertex.wesl` does them (or shadows would
// come loose). Fragment: `shading/fragment.wesl`.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};
import package::gpu::growth_common::{GrowthAttrs, grown_position, grown_wood_radius, leaf_growth_scale};
import package::shading::common::{flutter_offset, shake_at, wind_bend};

@if(FLORA_LEAF)
import package::shading::common::{leaf_has_fallen, leaf_hidden, leaf_season_tint, FALLEN_LEAF_CLIP};

@if(FLORA_FIRE && FLORA_LEAF)
import package::shading::common::{fire_at, leaf_burnt, leaf_fire_alpha};

@if(FLORA_FIRE && !FLORA_LEAF)
import package::shading::common::flora;

@if(FLORA_PUSH)
import package::shading::common::{push_offset, wood_flex};

@vertex
fn vertex(
    vertex_no_morph: Vertex,
    growth_attrs: GrowthAttrs,
@if(FLORA_LEAF)
    @location(11) sway: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    // Hidden leaves and flowers leave early, as in `shading/vertex.wesl`.
@if(FLORA_LEAF && VERTEX_COLORS) {
    if leaf_hidden(sway, vertex.color) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
@if(FLORA_LEAF && !VERTEX_COLORS) {
    if leaf_has_fallen(sway) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let origin = world_from_local[3].xyz;
    // The fire where the tree stands: a leaf it has burnt draws nothing.
    var fire = vec2<f32>(0.0);
@if(FLORA_FIRE && FLORA_LEAF) {
    fire = fire_at(origin);
    if leaf_burnt(sway, fire) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
    let shake = shake_at(origin, globals.time);
    var local = grown_position(growth_attrs);
    // A leaf flutters in proportion to how grown it is.
@if(FLORA_LEAF)
    local = local + flutter_offset(sway, origin, globals.time, shake) * leaf_growth_scale(growth_attrs);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );

    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.world_position = vec4<f32>(
        wind_bend(world.xyz, origin, length(world_from_local[1].xyz), globals.time, shake),
        1.0
    );
    // Pushed aside by a body moving through, as in `shading/vertex.wesl`, by the growing radius.
@if(FLORA_PUSH) {
    var flex = 0.0;
@if(FLORA_LEAF)
    flex = 1.0;
@if(!FLORA_LEAF)
    flex = wood_flex(grown_wood_radius(growth_attrs) * length(world_from_local[1].xyz));
    let pushed = out.world_position.xyz + push_offset(out.world_position.xyz, origin, flex);
    out.world_position = vec4<f32>(pushed, 1.0);
}
    out.position = position_world_to_clip(out.world_position.xyz);

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

    // Wood's second UV carries the vertex's radius (the bark fades in with it): the mesh holds the
    // mature one, so the growing radius replaces it.
@if(VERTEX_UVS_B && !FLORA_LEAF)
    out.uv_b = vec2<f32>(grown_wood_radius(growth_attrs), vertex.uv_b.y);
    // In a fire, its y carries how many trunk heights up the vertex is, as in `vertex.wesl`.
@if(VERTEX_UVS_B && FLORA_FIRE && !FLORA_LEAF)
    out.uv_b.y = local.y * flora.tree.x;

    // A deciduous tree's leaves turn their autumn colour, each at its own moment; a flowering
    // tree's flowers wear the blossom colour. The alpha (1 a leaf, 0 a flower, 0.25 a fruit) tells
    // the fragment stage which it draws; the opaque material ignores it otherwise.
@if(VERTEX_COLORS && !FLORA_LEAF)
    out.color = vertex.color;
@if(VERTEX_COLORS && FLORA_LEAF)
    out.color = vec4<f32>(vertex.color.rgb * leaf_season_tint(sway, vertex.color), vertex.color.a);
    // In a fire the alpha also carries how scorched the leaf is and how bright it burns.
@if(VERTEX_COLORS && FLORA_LEAF && FLORA_FIRE)
    out.color.a = leaf_fire_alpha(sway, vertex.color, fire);

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);

    return out;
}
