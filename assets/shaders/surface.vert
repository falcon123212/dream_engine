#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

layout(buffer_reference, std430) readonly buffer Geometry { float data[]; };
layout(buffer_reference, std430) readonly buffer Material { 
    vec3 base_color; float metallic; uint64_t emissive_ptr; float roughness; float ior; uvec2 _padding; 
};

layout(push_constant) uniform Constants {
    uint64_t geo_ptr;
    uint64_t mat_ptr;
    layout(offset = 16) mat4 model; // Correction de l'alignement
    layout(offset = 80) mat4 view_proj;
} pc;

layout(location = 0) out vec3 vColor;
layout(location = 1) out vec3 vNormal;

void main() {
    Geometry geo = Geometry(pc.geo_ptr);
    uint base_idx = gl_VertexIndex * 6;
    
    vec3 pos = vec3(geo.data[base_idx], geo.data[base_idx+1], geo.data[base_idx+2]);
    vec3 norm = vec3(geo.data[base_idx+3], geo.data[base_idx+4], geo.data[base_idx+5]);
    
    gl_Position = pc.view_proj * pc.model * vec4(pos, 1.0);
    vNormal = normalize(mat3(pc.model) * norm);
    
    // Placeholder si pas de matériaux (vu dans tes logs: Taille materials: 0)
    vColor = (pc.mat_ptr == 0) ? vec3(1.0, 1.0, 1.0) : Material(pc.mat_ptr).base_color;
}