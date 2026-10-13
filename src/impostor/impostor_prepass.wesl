// The shadow of a far tree (`ImpostorMaterial`): its quad turned toward the light as the main pass
// (`impostor.wesl`) turns it toward the camera, showing the view baked nearest the light's bearing,
// cut out by the picture's coverage less the leaves fallen this season. Only shadow maps draw it
// (the material's camera prepasses are off), and only for the pools that ask for it
// (`LodSettings::impostor_shadows`; the others' impostors are `NotShadowCaster`s). The quad stays in
// the trunk's plane here: moved to the front of the crown it would cast its shadow off the trunk.
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    render::{mesh_functions, mesh_view_bindings::view, view_transformations::position_world_to_clip},
};
import bevy_render::globals::Globals;
import package::shading::common::{flora, flora_wind, shake_at, wind_bend};
import package::impostor::impostor_common::impostor_corner;

@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

// The prepass's view bind group (view, globals, previous view); `bevy_pbr` has no WESL
// declaration of it.
@group(0) @binding(1) var<uniform> globals: Globals;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var albedo: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var surface: texture_2d<f32>;
// x: views in the atlas.
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<uniform> params: vec4<f32>;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
    let origin = world_from_local[3].xyz;

    // A directional light's shadow view is orthographic: the quad faces along its backward axis.
    let orthographic = view.clip_from_view[3].w == 1.0;
    let corner = impostor_corner(
        vertex.position.xy, world_from_local, view.world_position, view.world_from_view[2].xyz, orthographic, params.x,
        0.0,
    );
    let scale_y = length(world_from_local[1].xyz);
    let bent = wind_bend(corner.world, vertex.position.y, origin, scale_y, globals.time, shake_at(origin, globals.time));
    out.world_position = vec4<f32>(bent, 1.0);
    out.position = position_world_to_clip(bent);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(VERTEX_UVS_A)
    out.uv = vec2<f32>((corner.tile + vertex.uv.x) / params.x, vertex.uv.y);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, world_from_local[3]);
    return out;
}

// Whether the picture covers the texel at `uv`, less the leaves fallen this season (a deciduous
// tree's leafy texels, by their baked translucency), as the main pass thins the crown.
fn covered(uv: vec2<f32>) -> bool {
    let coverage = textureSample(albedo, atlas_sampler, uv).a;
    let leafy = saturate(textureSample(surface, atlas_sampler, uv).a) * flora.autumn.w;
    return coverage * (1.0 - flora_wind[5].y * leafy) >= 0.5;
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
@if(VERTEX_UVS_A) {
    if !covered(in.uv) {
        discard;
    }
}
    var out: FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
    return out;
}

// A depth-only shadow map (a point or spot light's, or a directional one where depth clipping can
// be turned off): the cut-out alone.
@else
@fragment
fn fragment(in: VertexOutput) {
@if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
@if(VERTEX_UVS_A) {
    if !covered(in.uv) {
        discard;
    }
}
}
