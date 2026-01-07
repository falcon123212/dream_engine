#version 460
#extension GL_EXT_buffer_reference : require

layout(location = 0) out vec3 outColor;

// On définit une référence vers nos données en VRAM (Pointeur)
layout(buffer_reference, std430) readonly buffer VertexBuffer {
    vec3 positions[];
};

// Push Constants : Le petit tunnel direct entre CPU et GPU
layout(push_constant) uniform Constants {
    VertexBuffer vertexBuffer;
} pc;

void main() {
    // On va chercher la position directement via le pointeur BDA !
    vec3 pos = pc.vertexBuffer.positions[gl_VertexIndex];
    gl_Position = vec4(pos, 1.0);
    outColor = vec3(1.0, 0.5, 0.0); // Orange Dream
}