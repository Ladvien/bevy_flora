// Shared by every flora shader: the wind, the flutter of a leaf, the surface of a leaf and the
// light that comes through it. The Rust side is `shading/mod.rs`.
//
// The wind follows Crysis' vegetation animation (GPU Gems 3, chapter 16): a main bend of the
// whole tree that grows with the square of the height and keeps every vertex's distance from the
// trunk, and per-leaf detail bending with smooth triangle waves of unrelated frequencies. The
// light through a leaf is Unreal's two-sided foliage model: the back side's wrapped N.L times a
// forward-scattering GGX lobe times the subsurface colour.
import bevy_pbr::render::{
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    pbr_lighting::D_GGX,
    pbr_ambient::ambient_light,
    shadows::fetch_directional_shadow,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    pbr_types::PbrInput,
};

struct FloraUniform {
    // 1 / trunk height, 0, 0, 0
    tree: vec4<f32>,
    // the species' sway, flutter, sway frequency, flutter frequency
    response: vec4<f32>,
    // translucency, roughness of the upper side, veins, underside
    leaf: vec4<f32>,
    // linear rgb of the light through a leaf, 0
    tint: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> flora: FloraUniform;

// The weather, the same for every tree (`FloraWindBuffer`, `wind_vectors` in shading/mod.rs):
//   0: wind direction x, direction z, strength, speed
//   1: bend, flutter, gust strength, turbulence
//   2: gust frequency, flutter frequency, 0, 0
//   3: gust front origin x, origin z, direction x, direction z
//   4: gust front speed, width, strength, start time
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage, read> flora_wind: array<vec4<f32>>;

fn smooth_curve(x: vec4<f32>) -> vec4<f32> {
    return x * x * (3.0 - 2.0 * x);
}

fn triangle_wave(x: vec4<f32>) -> vec4<f32> {
    return abs(fract(x + 0.5) * 2.0 - 1.0);
}

fn smooth_triangle_wave(x: vec4<f32>) -> vec4<f32> {
    return smooth_curve(triangle_wave(x));
}

// How far the top of a tree leans at full strength, per metre of trunk height.
const MAIN_BEND: f32 = 0.04;
// How far a leaf's tip flutters at full strength, per metre of its length.
const FLUTTER: f32 = 0.25;

// A phase of its own for each tree, from where it stands, so a forest does not sway in step.
fn tree_phase(origin: vec3<f32>) -> f32 {
    return fract(dot(origin, vec3<f32>(0.0713, 0.0, 0.0531)));
}

// How strong a gust front is at `origin`: a bump of the front's strength, `width` metres deep,
// that travels along the front's direction. 0 where there is no front.
fn gust_front(origin: vec3<f32>, time: f32) -> f32 {
    let front = flora_wind[3];
    let front_shape = flora_wind[4];
    if front_shape.z <= 0.0 {
        return 0.0;
    }
    let along = dot(origin.xz - front.xy, front.zw) - front_shape.x * (time - front_shape.w);
    let u = along / front_shape.y;
    return front_shape.z * exp(-u * u);
}

// The leaf's movement in the tree's local space. `sway`: the leaf's direction scaled by how far
// the vertex is from where the leaf is attached (xyz) and the leaf's phase (w).
fn flutter_offset(sway: vec4<f32>, origin: vec3<f32>, time: f32) -> vec3<f32> {
    let wind = flora_wind[0];
    let motion = flora_wind[1];
    let rate = flora_wind[2];
    let t = time * wind.w * 2.0 * rate.y * flora.response.w + sway.w + tree_phase(origin);
    let w = smooth_triangle_wave(vec4<f32>(t * 1.975, t * 0.793, 0.0, 0.0));
    let strength = wind.z + gust_front(origin, time);
    return sway.xyz * (FLUTTER * motion.y * flora.response.y * strength * (w.x + w.y - 1.0));
}

// `world` bent downwind with the tree. `local_y`: the vertex's height in the tree's own units,
// `scale_y`: the tree's scale along y.
fn wind_bend(world: vec3<f32>, local_y: f32, origin: vec3<f32>, scale_y: f32, time: f32) -> vec3<f32> {
    let wind = flora_wind[0];
    let motion = flora_wind[1];
    let rate = flora_wind[2];
    let front = flora_wind[3];
    let bump = gust_front(origin, time);
    if wind.z <= 0.0 && bump <= 0.0 {
        return world;
    }
    let t = time * wind.w * rate.x * flora.response.z;
    let phase = tree_phase(origin);
    let g = smooth_triangle_wave(vec4<f32>(t * 0.193 + phase, t * 0.375 + 1.7 * phase, t * 0.27 + 2.3 * phase, 0.0));
    // 0.6 to 1.0 of the wind at the default gust strength of 0.4.
    let gust = 1.0 - motion.z + motion.z * 0.5 * (g.x + g.y);
    let down = wind.xy * (wind.z * gust);
    let across = vec2<f32>(-wind.y, wind.x) * (wind.z * motion.w * (2.0 * g.z - 1.0));
    let push = down + across + front.zw * bump;
    let h = max(local_y, 0.0) * flora.tree.x;
    let reach = MAIN_BEND * motion.x * flora.response.x * (scale_y / max(flora.tree.x, 1.0e-4)) * h * h;
    let rel = world - origin;
    let moved = rel + vec3<f32>(push.x, 0.0, push.y) * reach;
    // Bend, do not stretch.
    return origin + moved * (length(rel) / max(length(moved), 1.0e-6));
}

fn sign_not_zero(v: vec2<f32>) -> vec2<f32> {
    return select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

// A unit vector as two numbers in -1..1: the octahedral mapping.
fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        return (1.0 - abs(p.yx)) * sign_not_zero(p);
    }
    return p;
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var v = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if v.z < 0.0 {
        let folded = (1.0 - abs(v.yx)) * sign_not_zero(v.xy);
        v = vec3<f32>(folded.x, folded.y, v.z);
    }
    return normalize(v);
}

// The light a surface lets through from the far side of `N`: every directional light that shines
// on the back, scattered toward the viewer. `through`: the colour the light takes (the leaf's
// colour, its tint and its translucency). `visibility`: how much of the sky the leaf sees, for the
// ambient light that comes through. Point and spot lights do not shine through.
fn transmitted_light(
    P: vec4<f32>,
    world_normal: vec3<f32>,
    N: vec3<f32>,
    V: vec3<f32>,
    frag_coord: vec4<f32>,
    mesh_flags: u32,
    shadow_factor: f32,
    through: vec3<f32>,
    visibility: f32,
) -> vec3<f32> {
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), P);

    var sum = vec3<f32>(0.0);
    let n_directional_lights = view_bindings::lights.n_directional_lights;
    for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        let L = (*light).direction_to_light;
        let wrap = saturate((-dot(N, L) + 0.5) / 2.25);
        let scatter = D_GGX(0.6, saturate(-dot(V, L)));
        var shadow = 1.0;
        if (mesh_flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            // The shadow of what is in front of the light, seen from the back of the leaf.
            shadow = fetch_directional_shadow(i, P, -world_normal, view_z, frag_coord.xy) * shadow_factor;
        }
        sum = sum + (*light).color.rgb * (wrap * scatter * shadow);
    }
    return sum * through
        + ambient_light(P, -N, -V, 1.0, through, vec3<f32>(0.0), 1.0, vec3<f32>(visibility));
}

// How much of a leaf's specular reflection to keep, seen along `V` (from the surface toward the
// camera): all of it unless the camera looks toward a directional light, none when it looks
// straight into it. A leaf card stands for a leaf, or a cluster of needles, whose many small
// surfaces would scatter the sun's glint; a flat card seen against the sun reflects all of it
// into the camera at once, and the crown's top turns white, as if it were snow or dead. Facing
// the sun, a crown glows green through its leaves (`transmitted_light`) instead. Lit from the
// side or from behind the camera nothing changes.
fn leaf_gloss_toward_light(V: vec3<f32>) -> f32 {
    var toward = 0.0;
    for (var i: u32 = 0u; i < view_bindings::lights.n_directional_lights; i = i + 1u) {
        toward = max(toward, -dot(V, view_bindings::lights.directional_lights[i].direction_to_light));
    }
    return 1.0 - smoothstep(0.4, 0.8, toward);
}

// The surface of a leaf card, in place: veins, hue of the underside, gloss, crown occlusion.
// `uv`: the card's own (0..1 across and from base to tip), `shade`: x is 1 on the upper side,
// y is how much of the sky the leaf sees.
fn leaf_surface(p: ptr<function, PbrInput>, uv: vec2<f32>, shade: vec2<f32>) {
    // Derivatives before any branch.
    let x = abs(uv.x - 0.5) * 2.0;
    let y = uv.y;
    let aa = max(fwidth(x), fwidth(y));

    var color = (*p).material.base_color.rgb;
    if flora.leaf.z > 0.0 {
        // Midrib, thicker at the base, and side veins running up toward the edge; both fade
        // out where they are finer than a pixel.
        let mid_w = mix(0.08, 0.02, y);
        let midrib = 1.0 - smoothstep(mid_w, mid_w + aa, x);
        let t = fract(y * 5.0 - x * 1.6);
        let along = min(t, 1.0 - t);
        let side = (1.0 - smoothstep(0.04, 0.04 + 5.0 * aa, along))
            * smoothstep(0.1, 0.2, x)
            * (1.0 - smoothstep(0.8, 0.95, x));
        let vein = max(midrib, 0.6 * side) * flora.leaf.z * saturate(1.0 - 20.0 * aa);
        color = mix(color, color * vec3<f32>(1.35, 1.3, 0.75), vein) * mix(0.92, 1.05, y);
    }

    // The lower side is paler and less saturated.
    let upper = shade.x > 0.5;
    if !upper {
        let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        color = mix(color, vec3<f32>(luminance), 0.25) * (1.0 + flora.leaf.w);
    }

    (*p).material.base_color = vec4<f32>(color, (*p).material.base_color.a);
    (*p).material.perceptual_roughness = select(min(flora.leaf.y + 0.3, 1.0), flora.leaf.y, upper);
    (*p).material.reflectance = (*p).material.reflectance * leaf_gloss_toward_light((*p).V);
    (*p).diffuse_occlusion = (*p).diffuse_occlusion * shade.y;
    (*p).specular_occlusion = (*p).specular_occlusion * shade.y;
}

fn leaf_transmission(p: PbrInput, visibility: f32) -> vec3<f32> {
    return transmitted_light(
        p.world_position,
        p.world_normal,
        p.N,
        p.V,
        p.frag_coord,
        p.flags,
        p.directional_shadow_factor,
        p.material.base_color.rgb * flora.tint.rgb * flora.leaf.x,
        visibility,
    );
}
