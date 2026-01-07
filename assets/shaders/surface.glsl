#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

// Structures pour le Bindless
layout(buffer_reference, std430) readonly buffer Geometry { float vertices[]; };
layout(buffer_reference, std430) readonly buffer Material { vec3 color; };

layout(push_constant) uniform Constants {
    uint64_t geo_ptr;
    uint64_t mat_ptr;
    mat4 model;
    mat4 view_proj;
} pc;

#ifdef VERTEX_SHADER
void main() {
    // Un triangle de secours visible
    vec3 positions[3] = vec3[](
        vec3(0.0, -0.5, 0.0),
        vec3(0.5, 0.5, 0.0),
        vec3(-0.5, 0.5, 0.0)
    );
    gl_Position = vec4(positions[gl_VertexIndex], 1.0);
}
#endif

#ifdef FRAGMENT_SHADER
layout(location = 0) out vec4 outColor;
void main() {
    // Couleur Cyan pour confirmer que le shader fonctionne
    outColor = vec4(0.0, 0.8, 1.0, 1.0);
}
#endif