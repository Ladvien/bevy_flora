// Main-pass vertex stage of a tree drawn with `FloraMaterial`: Bevy's own, with the leaves
// fluttering and the whole tree bending in the wind. Fragment: `fragment.wesl`. The prepass
// (depth, shadow maps) does the same in `prepass.wesl`, or shadows would come loose.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};
import package::shading::common::{flutter_offset, shake_at, wind_bend};

@if(FLORA_LEAF)
import package::shading::common::{
    leaf_has_fallen, leaf_hidden, leaf_glide_tint, leaf_defoliated, leaf_withering, leaf_withered_tint, tags_read,
    FALLEN_LEAF_CLIP,
};

@if(FLORA_LEAF && FLORA_GLIDE)
import package::shading::common::{gliding, glide_tint};

@if(FLORA_FIRE && FLORA_LEAF)
import package::shading::common::{fire_at, leaf_burnt, leaf_fire_alpha};

@if(FLORA_FIRE && !FLORA_LEAF)
import package::shading::common::flora;

@if(FLORA_PUSH)
import package::shading::common::{push_offset, wood_flex};

@vertex
fn vertex(
    vertex_no_morph: Vertex,
@if(FLORA_LEAF)
    @location(11) sway: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    // A fallen leaf or a flower out of season draws nothing, nor does a leaf a sick or dying tree
    // has lost (`TreeCondition`, in the mesh's tag): its vertices leave the clip volume before any
    // of the wind's work, so a hidden leaf costs little more than its fetch.
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
    // The fire where the tree stands (`FloraFire`): a leaf it has burnt draws nothing.
    var fire = vec2<f32>(0.0);
@if(FLORA_FIRE && FLORA_LEAF) {
    fire = fire_at(origin);
    if leaf_burnt(sway, fire) {
        out.position = FALLEN_LEAF_CLIP;
        return out;
    }
}
    let shake = shake_at(origin, globals.time);
    var local = vertex.position;
@if(FLORA_LEAF)
    local = local + flutter_offset(sway, origin, globals.time, shake);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );

    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.world_position = vec4<f32>(
        wind_bend(world.xyz, origin, globals.time, shake),
        1.0
    );
    // A body moving through the plant (`FoliagePusher`): leaves and thin wood give way.
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

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
    // In a fire, wood's second UV's y (1 in the mesh) carries how many trunk heights up the vertex
    // is: the char climbs from the ground.
@if(VERTEX_UVS_B && FLORA_FIRE && !FLORA_LEAF)
    out.uv_b.y = local.y * flora.tree.x;

    // A deciduous tree's leaves turn their autumn colour, each at its own moment; a flowering
    // tree's flowers wear the blossom colour. The alpha (1 a leaf, 0 a flower, 0.25 a fruit) tells
    // the fragment stage which it draws; the opaque material ignores it otherwise.
@if(VERTEX_COLORS && !FLORA_LEAF)
    out.color = vertex.color;
    // A pooled tree's look that glides (`RecolorPool::glide`): its leaves take the look's blend. A
    // sick or dying tree's leaves, those that are left, wither toward the dry leaf colour.
@if(VERTEX_COLORS && FLORA_LEAF) {
    var glide = vec3<f32>(1.0);
@if(FLORA_GLIDE)
    if gliding() {
        glide = glide_tint(tag, true);
    }
    out.color = vec4<f32>(
        vertex.color.rgb * leaf_withered_tint(
            leaf_glide_tint(sway, vertex.color, glide), leaf_withering(sway, vertex.color.a, tag),
        ),
        vertex.color.a,
    );
}
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
