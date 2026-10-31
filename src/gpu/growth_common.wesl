// Where a growing vertex goes. The Rust mirror is `gpu/frame.rs`; the buffer
// layout is written by `GrowthState::encode` in `gpu/state.rs`.
//
// The buffer is an array of vec4: a header [node count, ring count, trunk flags,
// cone lift], then 5 vec4 per node, then 2 vec4 per trunk ring.

struct GrowthAttrs {
    // Node id; placement class in the low 8 bits and a leaf's ordinal above them.
    @location(8) id: vec2<u32>,
    // Anchor on the node's axis (xyz, in node lengths); fraction along the piece or trunk ring (w).
    @location(9) axis: vec4<f32>,
    // Offset from the anchor (xyz, in units of the piece's scale); trunk root bulge unit (w).
    @location(10) offset: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> growth: array<vec4<f32>>;

const KIND_TUBE: u32 = 0u;
const KIND_LEAF: u32 = 1u;
const KIND_COLLAR: u32 = 2u;
const KIND_BASE: u32 = 3u;
const KIND_BASE_COLLAR: u32 = 4u;
const KIND_ROOT_FAR: u32 = 5u;
const KIND_ROOT_FAR_COLLAR: u32 = 6u;
const KIND_TRUNK: u32 = 7u;
const KIND_TRUNK_TOP_CAP: u32 = 8u;
const KIND_TRUNK_CONE: u32 = 9u;

const WOOD: u32 = 1u;
const COLLAR: u32 = 2u;
const BASE: u32 = 4u;
const BASE_COLLAR: u32 = 8u;

const TOP_CAP: u32 = 1u;
const CONE: u32 = 2u;

const NODE_VEC4S: u32 = 5u;
const RING_VEC4S: u32 = 2u;

// `v` turned about the Y axis by `twist` radians, the way a trunk ring's twist
// turns its vertices.
fn rotate_y(v: vec3<f32>, twist: f32) -> vec3<f32> {
    let s = sin(twist);
    let c = cos(twist);
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

// How far the trunk vertex `g`, on the ring `ring_a`/`ring_b`, sits from the trunk's axis in
// units of its offset: the ring's taper radius plus the root bulge at the vertex's angle.
fn trunk_scale(g: GrowthAttrs, ring_a: vec4<f32>, ring_b: vec4<f32>) -> f32 {
    return ring_a.w + g.offset.w * ring_b.x;
}

// Where the vertex's trunk ring starts in the growth buffer.
fn trunk_ring_at(g: GrowthAttrs, node_count: u32) -> u32 {
    return 1u + node_count * NODE_VEC4S + u32(g.axis.w + 0.5) * RING_VEC4S;
}

fn trunk_position(g: GrowthAttrs, kind: u32, node_count: u32, trunk_flags: u32, cone_lift: f32) -> vec3<f32> {
    let at = trunk_ring_at(g, node_count);
    let ring_a = growth[at];
    let ring_b = growth[at + 1u];
    let center = ring_a.xyz;
    let scale = trunk_scale(g, ring_a, ring_b);
    let twist = ring_b.y;
    if kind == KIND_TRUNK_CONE {
        if (trunk_flags & CONE) != 0u {
            return center + vec3<f32>(0.0, cone_lift, 0.0) + g.offset.xyz;
        }
        return center + scale * rotate_y(g.axis.xyz, twist);
    }
    if kind == KIND_TRUNK_TOP_CAP && (trunk_flags & TOP_CAP) == 0u {
        return center;
    }
    return center + scale * rotate_y(g.offset.xyz, twist);
}

// What a node's piece (`kind`, not a trunk's) scales its vertex's offset by at the current growth
// state: the radius of wood, the size of a leaf. `b`, `c` and `d` are the node's second to
// fourth vec4s (see `grown_position`).
fn piece_scale(g: GrowthAttrs, kind: u32, b: vec4<f32>, c: vec4<f32>, d: vec4<f32>) -> f32 {
    var scale = 0.0;
    switch kind {
        case KIND_TUBE: {
            scale = mix(b.x, b.y, g.axis.w);
        }
        case KIND_LEAF: {
            // `b.w` is the real number of leaves the node carries (negative: all of them, at full
            // size). The leaf of this ordinal is absent before its turn and eases in over the
            // growth of branch the next one takes to be born: `leaf_flush_scale` in `frame.rs`.
            let ordinal = g.id.y >> 8u;
            let flush = select(clamp(b.w - f32(ordinal), 0.0, 1.0), 1.0, b.w < 0.0);
            let shown = b.z > 0.0 && flush > 0.0;
            scale = select(0.0, b.z * smoothstep(0.0, 1.0, flush), shown);
        }
        case KIND_COLLAR: {
            scale = mix(c.w, d.w, smoothstep(0.0, 1.0, g.axis.w));
        }
        case KIND_BASE, KIND_ROOT_FAR: {
            scale = b.x;
        }
        default: {
            // KIND_BASE_COLLAR, KIND_ROOT_FAR_COLLAR
            scale = c.w;
        }
    }
    return scale;
}

// The vertex's position in the tree's local space at the current growth state.
fn grown_position(g: GrowthAttrs) -> vec3<f32> {
    let kind = g.id.y & 255u;
    let header = growth[0];
    let node_count = u32(header.x);
    if kind >= KIND_TRUNK {
        return trunk_position(g, kind, node_count, u32(header.z), header.w);
    }

    let at = 1u + g.id.x * NODE_VEC4S;
    let a = growth[at];       // start, length
    let b = growth[at + 1u];  // base radius, tip radius, leaf scale, leaf count
    let c = growth[at + 2u];  // collar start, radius
    let d = growth[at + 3u];  // collar end, radius
    let e = growth[at + 4u];  // root, flags
    let flags = u32(e.w);
    let start = a.xyz;
    let on_axis = start + a.w * g.axis.xyz;

    var anchor = on_axis;
    var on = true;
    switch kind {
        case KIND_TUBE: {
            on = (flags & WOOD) != 0u;
        }
        case KIND_LEAF: {
            on = true;
        }
        case KIND_COLLAR: {
            anchor = mix(c.xyz, d.xyz, g.axis.w);
            on = (flags & COLLAR) != 0u;
        }
        case KIND_BASE: {
            anchor = start;
            on = (flags & BASE) != 0u;
        }
        case KIND_BASE_COLLAR: {
            anchor = c.xyz;
            on = (flags & BASE_COLLAR) != 0u;
        }
        case KIND_ROOT_FAR: {
            anchor = e.xyz;
            on = (flags & BASE) != 0u;
        }
        default: {
            // KIND_ROOT_FAR_COLLAR
            anchor = e.xyz;
            on = (flags & BASE_COLLAR) != 0u;
        }
    }
    // A piece that does not exist yet collapses onto the node's start (a leaf
    // with no size already sits on its anchor).
    return select(start, anchor + piece_scale(g, kind, b, c, d) * g.offset.xyz, on);
}

// The radius, in metres, of the wood vertex `g` at the current growth state: the scale
// `grown_position` gives its (radial) offset. 0 for a leaf's vertex and for the apex of a
// mature pointed trunk. The bark fades in with it.
fn grown_wood_radius(g: GrowthAttrs) -> f32 {
    let kind = g.id.y & 255u;
    if kind == KIND_LEAF {
        return 0.0;
    }
    let header = growth[0];
    let node_count = u32(header.x);
    if kind >= KIND_TRUNK {
        if kind == KIND_TRUNK_CONE && (u32(header.z) & CONE) != 0u {
            return 0.0;
        }
        let at = trunk_ring_at(g, node_count);
        return trunk_scale(g, growth[at], growth[at + 1u]);
    }
    let at = 1u + g.id.x * NODE_VEC4S;
    return piece_scale(g, kind, growth[at + 1u], growth[at + 2u], growth[at + 3u]);
}

// How grown the vertex's leaf is: the share of its mature size that `grown_position` scales
// a leaf by; 0 for a vertex that is not a leaf's. A leaf's flutter scales by it too.
fn leaf_growth_scale(g: GrowthAttrs) -> f32 {
    if (g.id.y & 255u) != KIND_LEAF {
        return 0.0;
    }
    let at = 1u + g.id.x * NODE_VEC4S;
    return piece_scale(g, KIND_LEAF, growth[at + 1u], growth[at + 2u], growth[at + 3u]);
}
