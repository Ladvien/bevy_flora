// Fragment stage of a tree drawn with `FloraMaterial` or `GrowthMaterial` (main pass, forward
// only): Bevy's `pbr.wesl` with the leaf's surface before lighting and the light through the leaf
// after it. Wood (no FLORA_LEAF) is StandardMaterial's.
import bevy_pbr::{
    render::{
        pbr_functions::alpha_discard,
        pbr_fragment::pbr_input_from_standard_material,
        forward_io::{VertexOutput, FragmentOutput},
        pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
        mesh_view_bindings::view,
    },
    decal::clustered::apply_decals,
};

@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@if(FLORA_LEAF)
import package::shading::common::{leaf_surface, leaf_transmission};

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;

    // In the crossfade section of a visibility range, discard according to the dither pattern.
@if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );

@if(FLORA_LEAF)
    leaf_surface(&pbr_input, in.uv, in.uv_b);

    apply_decals(&pbr_input);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
@if(FLORA_LEAF)
    out.color = vec4<f32>(
        out.color.rgb + view.exposure * leaf_transmission(pbr_input, in.uv_b.y),
        out.color.a
    );
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
