// Fullscreen-triangle SDF renderer for a single rounded rectangle.
// The UI layer will grow this into a quad batcher; the math stays the same.

struct Params {
    clear: vec4<f32>,
    rect: vec4<f32>,   // x, y, width, height in physical pixels
    color: vec4<f32>,
    radius: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
};

@group(0) @binding(0) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // Oversized triangle covering the whole viewport in clip space.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

fn rounded_box_distance(point: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let half_size = params.rect.zw * 0.5;
    let center = params.rect.xy + half_size;
    let distance = rounded_box_distance(position.xy - center, half_size, params.radius);

    // One-pixel analytic antialiasing around the shape boundary.
    let coverage = 1.0 - smoothstep(0.0, 1.0, distance);

    let alpha = mix(params.clear.a, params.color.a, coverage);
    let rgb = mix(params.clear.rgb, params.color.rgb, coverage);

    // Premultiplied output; opaque compositors ignore the alpha channel.
    return vec4<f32>(rgb * alpha, alpha);
}
