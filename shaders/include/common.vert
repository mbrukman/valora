#version 150

uniform float scale;
uniform vec2 center;
uniform vec2 root_center;

vec4 tweened_position(vec2 position) {
    vec2 center_offset = (position - root_center) * scale;
    return vec4(center_offset + center, 0.0, 1.0);
}

vec2 tex_position(vec4 position) {
    return (vec2(position[0], position[1]) + 1.0) / 2.0;
}