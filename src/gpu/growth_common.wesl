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

fn trunk_position(g: GrowthAttrs, kind: u32, node_count: u32, trunk_flags: u32, cone_lift: f32) -> vec3<f32> {
    let ring = u32(g.axis.w + 0.5);
    let at = 1u + node_count * NODE_VEC4S + ring * RING_VEC4S;
    let ring_a = growth[at];
    let ring_b = growth[at + 1u];
    let center = ring_a.xyz;
    let scale = ring_a.w + g.offset.w * ring_b.x;
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
    var scale = 0.0;
    var on = true;
    switch kind {
        case KIND_TUBE: {
            scale = mix(b.x, b.y, g.axis.w);
            on = (flags & WOOD) != 0u;
        }
        case KIND_LEAF: {
            let ordinal = g.id.y >> 8u;
            let shown = b.z > 0.0 && (b.w < 0.0 || f32(ordinal) < b.w);
            scale = select(0.0, b.z, shown);
        }
        case KIND_COLLAR: {
            anchor = mix(c.xyz, d.xyz, g.axis.w);
            scale = mix(c.w, d.w, smoothstep(0.0, 1.0, g.axis.w));
            on = (flags & COLLAR) != 0u;
        }
        case KIND_BASE: {
            anchor = start;
            scale = b.x;
            on = (flags & BASE) != 0u;
        }
        case KIND_BASE_COLLAR: {
            anchor = c.xyz;
            scale = c.w;
            on = (flags & BASE_COLLAR) != 0u;
        }
        case KIND_ROOT_FAR: {
            anchor = e.xyz;
            scale = b.x;
            on = (flags & BASE) != 0u;
        }
        default: {
            // KIND_ROOT_FAR_COLLAR
            anchor = e.xyz;
            scale = c.w;
            on = (flags & BASE_COLLAR) != 0u;
        }
    }
    // A piece that does not exist yet collapses onto the node's start (a leaf
    // with no size already sits on its anchor).
    return select(start, anchor + scale * g.offset.xyz, on);
}
