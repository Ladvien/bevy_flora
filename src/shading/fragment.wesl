// Fragment stage of a tree drawn with `FloraMaterial` or `GrowthMaterial` (main pass, forward
// only): Bevy's `pbr.wesl` with the leaf's surface and the season's snow before lighting and the
// light through the leaf after it. Wood (no FLORA_LEAF) is StandardMaterial's, under the snow.
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

@if(FLORA_DETAIL && !FLORA_LEAF)
import package::shading::common::bark_surface;

@if(FLORA_DETAIL && FLORA_LEAF)
import package::shading::common::leaf_detail;

@if(FLORA_TOON)
import package::shading::common::{toon_lighting, toon_rim};

@if(FLORA_LEAF)
import package::shading::common::{leaf_surface, leaf_transmission};

import package::shading::common::{snow_on, snow_shelter, facing_normal, LEAF_FROST};

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

@if(FLORA_DETAIL && !FLORA_LEAF)
    bark_surface(&pbr_input, in.uv, in.uv_b.x);

    // A flower (vertex alpha 0, `FoliageParams::blossom`) is a plain petal: no leaf scan, veins or
    // leaf gloss.
@if(FLORA_LEAF && VERTEX_COLORS)
    let flower = in.color.a < 0.5;
@if(FLORA_LEAF && !VERTEX_COLORS)
    let flower = false;

@if(FLORA_DETAIL && FLORA_LEAF)
    if !flower {
        leaf_detail(&pbr_input, in.uv);
    }

@if(FLORA_LEAF)
    if !flower {
        leaf_surface(&pbr_input, in.uv, in.uv_b);
    }

    // The season's snow (none in summer, which leaves the surface as it was): on the side of a leaf
    // card the camera sees, if that side faces up, and a dusting on every leaf, as much as the leaf
    // sees of the sky; on the wood, by its own normal. A leaf under snow lets no light through.
@if(FLORA_LEAF)
    let snowed = snow_on(
        &pbr_input,
        facing_normal(pbr_input.world_position.xyz, pbr_input.V),
        snow_shelter(in.uv_b.y),
        LEAF_FROST,
    );
@else
    let snowed = snow_on(&pbr_input, pbr_input.world_normal, 1.0, 0.0);

    apply_decals(&pbr_input);

    var out: FragmentOutput;
@if(FLORA_TOON)
    out.color = toon_lighting(pbr_input);
@else
    out.color = apply_pbr_lighting(pbr_input);
@if(FLORA_LEAF)
    out.color = vec4<f32>(
        out.color.rgb + view.exposure * leaf_transmission(pbr_input, in.uv_b.y, flower) * (1.0 - snowed),
        out.color.a
    );
@if(FLORA_TOON)
    out.color = vec4<f32>(out.color.rgb * toon_rim(pbr_input.world_normal, pbr_input.V), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
