// Prepass fragment stage of a flora material while a camera sees through the trees
// (`SeeThroughTrees`): `specialize_flora` puts it in a camera's prepass pipelines then, in place
// of Bevy's `pbr_prepass.wesl` where they have a fragment stage and as their only one where a
// depth-only prepass has none, so a prepass keeps no depth (or normal, or motion) where the main
// pass cut the tree. Shadow maps keep the whole tree: `see_through` cuts only in the fading
// camera's own view.
//
// The outputs are Bevy's (bevy_pbr 0.20's `render/pbr_prepass.wesl`, MIT OR Apache-2.0, without
// its meshlet branches): the same normal, motion vector and unclipped depth.
import bevy_pbr::{
    render::{
        pbr_prepass_functions,
        pbr_bindings,
        pbr_types,
        pbr_functions,
        pbr_functions::SampleBias,
        mesh_bindings::mesh,
        mesh_view_bindings::view,
    },
    prepass::io as prepass_io,
};
import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};
import package::shading::common::see_through;

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> prepass_io::FragmentOutput {
@if(BINDLESS)
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
@if(BINDLESS)
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
@if(BINDLESS)
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
@if(!BINDLESS)
    let flags = pbr_bindings::material.flags;
@if(!BINDLESS)
    let uv_transform = pbr_bindings::material.uv_transform;

    // If we're in the crossfade section of a visibility range, conditionally
    // discard the fragment according to the visibility pattern.
@if(VISIBILITY_RANGE_DITHER)
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);

    see_through(in.world_position.xyz, in.position.xy, view.world_position);

    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);

    var out: prepass_io::FragmentOutput;

@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;

    // NOTE: Unlit bit not set means == 0 is true, so the true case is if lit
@if(NORMAL_PREPASS)
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let mesh_flags = mesh[in.instance_index].flags;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);

        let world_normal = pbr_functions::prepare_world_normal(
            in.world_normal,
            double_sided,
            is_logically_front,
        );

        var normal = world_normal;

@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
@if(STANDARD_MATERIAL_NORMAL_MAP) {
@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
@else
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;

        // Fill in the sample bias so we can sample from textures.
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;

@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
@else
        let texture = pbr_bindings::normal_map_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
@else
        let sampler_ = pbr_bindings::normal_map_sampler;

        let Nt = textureSampleBias(texture, sampler_, uv, bias.mip_bias).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);

        normal = pbr_functions::apply_normal_mapping(
            flags,
            TBN,
            double_sided,
            is_logically_front,
            Nt,
        );
}
}
}

        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);

    return out;
}

// A depth-only prepass (or a point or spot light's shadow map, which shares its pipeline): the
// cut alone.
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    see_through(in.world_position.xyz, in.position.xy, view.world_position);
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
}
