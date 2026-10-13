// Where a far tree's quad stands (`ImpostorMaterial`): the placement the main pass
// (`impostor.wesl`) and the shadow pass (`impostor_prepass.wesl`) share.
//
// The quad is built in the tree's local space with the picture's frame: x across, y up, z toward
// the viewer. `world_from_local` carries the tree's position, yaw and size; only the position and
// the size place it, the yaw picks the view.

const TAU: f32 = 6.283185307179586;

struct ImpostorCorner {
    // The corner in the world, before the wind.
    world: vec3<f32>,
    // The baked view it shows, 0 to the number of views less one.
    tile: f32,
}

// Corner `corner` (x across, y up the quad, in the tree's units) of the quad of the tree at
// `world_from_local`, seen from `eye` (a perspective view) or along `back` (an orthographic one,
// whose rays are parallel; `back` points from the scene toward the view), showing the nearest of
// `views` baked views. The quad turns about the trunk to face the viewer along the ground. With
// `reach` above 0 it is drawn that far before the trunk (times the tree's size), each corner moved
// toward the viewer along its own ray, so the picture on screen is the same and its foot, the near
// edge of a crown seen from above, is not under the ground.
fn impostor_corner(
    corner: vec2<f32>,
    world_from_local: mat4x4<f32>,
    eye: vec3<f32>,
    back: vec3<f32>,
    orthographic: bool,
    views: f32,
    reach: f32,
) -> ImpostorCorner {
    let origin = world_from_local[3].xyz;
    let scale_x = length(world_from_local[0].xyz);
    let scale_y = length(world_from_local[1].xyz);
    let scale_z = length(world_from_local[2].xyz);

    // Toward the viewer, along the ground.
    var to_viewer = eye - origin;
    if orthographic {
        to_viewer = back;
    }
    var flat = vec3<f32>(to_viewer.x, 0.0, to_viewer.z);
    if dot(flat, flat) < 1.0e-8 {
        flat = vec3<f32>(0.0, 0.0, 1.0);
    }
    flat = normalize(flat);

    // The quad's right: up cross toward-the-viewer, the viewer's own right.
    let right = vec3<f32>(flat.z, 0.0, -flat.x);
    var world = origin + right * (corner.x * scale_x) + vec3<f32>(0.0, corner.y * scale_y, 0.0);

    // The view: the yaw of the viewer in the tree's own frame, to the nearest of the baked ones
    // (view k looks from the local +Z turned k / views of a turn about Y).
    let local_x = dot(to_viewer, world_from_local[0].xyz) / (scale_x * scale_x);
    let local_z = dot(to_viewer, world_from_local[2].xyz) / (scale_z * scale_z);
    let view_index = floor(atan2(local_x, local_z) / TAU * views + 0.5);

    if reach > 0.0 {
        var ray = back;
        if !orthographic {
            ray = normalize(eye - world);
        }
        world = world + ray * (reach * max(scale_x, scale_z) / max(dot(ray, flat), 0.2));
    }
    var out: ImpostorCorner;
    out.world = world;
    out.tile = view_index - views * floor(view_index / views);
    return out;
}
