#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

// --- RESSOURCES PARTAGÉES ---

layout(buffer_reference, std430) readonly buffer Geometry { float data[]; };
layout(buffer_reference, std430) readonly buffer Material { 
    vec3 base_color; float metallic; uint64_t emissive_ptr; 
    float roughness; float ior; uvec2 _padding; 
};

// Image pour stocker l'historique et lisser le rendu
layout(set = 0, binding = 0, rgba32f) uniform image2D accum_buffer;

layout(push_constant) uniform Constants {
    uint64_t geo_ptr;    // 0..8
    uint64_t mat_ptr;    // 8..16
    uint frame_index;    // 16..20
    // Padding 20..32
    layout(offset = 32) vec3 cam_pos; // 32..44
    // Padding 44..48
    layout(offset = 48) mat4 model;      // 48..112
    layout(offset = 112) mat4 view_proj; // 112..176
} pc;

// Générateur de Bruit Pseudo-Aléatoire (Gold Noise)
float random(vec2 st) {
    return fract(sin(dot(st.xy, vec2(12.9898,78.233))) * 43758.5453123);
}

// --- VERTEX SHADER ---
#ifdef VERTEX_SHADER
layout(location = 0) out vec3 vColor;
layout(location = 1) out vec3 vNormal;
layout(location = 2) out vec3 vWorldPos;
layout(location = 3) out float vMetallic;
layout(location = 4) out float vRoughness;

void main() {
    Geometry geo = Geometry(pc.geo_ptr);
    uint base_idx = gl_VertexIndex * 6;
    
    vec3 pos = vec3(geo.data[base_idx], geo.data[base_idx+1], geo.data[base_idx+2]);
    vec3 norm = vec3(geo.data[base_idx+3], geo.data[base_idx+4], geo.data[base_idx+5]);
    
    vec4 world_pos = pc.model * vec4(pos, 1.0);
    vWorldPos = world_pos.xyz;
    gl_Position = pc.view_proj * world_pos;
    
    // 🌀 MICRO-JITTERING (Anti-Aliasing Temporel)
    if (pc.frame_index > 0) {
        float jitter_x = (random(vec2(pc.frame_index, gl_VertexIndex)) - 0.5) * 0.001;
        float jitter_y = (random(vec2(gl_VertexIndex, pc.frame_index)) - 0.5) * 0.001;
        gl_Position.xy += vec2(jitter_x, jitter_y) * gl_Position.w;
    }

    float dist = length(vWorldPos - pc.cam_pos); 
    // Ajustement de la taille des points pour qu'ils se touchent sans trop se superposer
// ✅ PLUS FIN -> Effet Sable / Poudre
gl_PointSize = (1400.0 / max(dist, 0.1)) * 0.008;     
    vNormal = normalize(mat3(pc.model) * norm);
    
    // Extraction Matériau
    if (pc.mat_ptr != 0) {
        Material mat = Material(pc.mat_ptr);
        vColor = mat.base_color;
        vMetallic = mat.metallic;
        vRoughness = mat.roughness;
    } else {
        vColor = vec3(1.0, 0.84, 0.0); // Gold defaut
        vMetallic = 1.0;
        vRoughness = 0.2;
    }
}
#endif

// --- FRAGMENT SHADER ---
#ifdef FRAGMENT_SHADER
layout(location = 0) in vec3 vColor;
layout(location = 1) in vec3 vNormal; // Normale globale (forme)
layout(location = 2) in vec3 vWorldPos;
layout(location = 3) in float vMetallic;
layout(location = 4) in float vRoughness;

layout(location = 0) out vec4 outColor;

const float PI = 3.14159265359;

// Fresnel Schlick
vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Distribution GGX
float DistributionGGX(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;
    
    float num = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
    
    return num / denom;
}

// Geometry Smith
float GeometrySchlickGGX(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;
    float num = NdotV;
    float denom = NdotV * (1.0 - k) + k;
    return num / denom;
}

float GeometrySmith(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2 = GeometrySchlickGGX(NdotV, roughness);
    float ggx1 = GeometrySchlickGGX(NdotL, roughness);
    return ggx1 * ggx2;
}

void main() {
    // 1. Coordonnées locales du point (-1 à 1)
    vec2 uv = gl_PointCoord * 2.0 - 1.0;
    float mag = dot(uv, uv);
    
    // On découpe en cercle parfait
    if (mag > 1.0) discard;

    // 2. --- SPHERE IMPOSTOR (Correction Volume) ---
    // Calcul de la normale locale du voxel sphérique
    vec3 N_voxel = vec3(uv, sqrt(1.0 - mag)); 

    // Vue
    vec3 V = normalize(pc.cam_pos - vWorldPos);
    
    // Perturbation de la normale globale avec la courbure locale
    // C'est ici que l'effet "strate" disparaît
    vec3 N_perturbed = normalize(vNormal + (N_voxel * 0.7)); 
    vec3 N = N_perturbed;

    // Lumière Directionnelle
    vec3 L = normalize(vec3(0.5, 1.0, 0.3)); 
    vec3 H = normalize(V + L);
    vec3 radiance = vec3(3.0); 

    // PBR Paramètres
    vec3 albedo = pow(vColor, vec3(2.2));
    float metallic = vMetallic;
    // Lissage des bords du voxel pour éviter le moiré
    float roughness = mix(vRoughness, 1.0, pow(mag, 4.0));

    vec3 F0 = vec3(0.04); 
    F0 = mix(F0, albedo, metallic);

    // Cook-Torrance BRDF
    float NDF = DistributionGGX(N, H, roughness);    
    float G = GeometrySmith(N, V, L, roughness);       
    vec3 F = fresnelSchlick(max(dot(H, V), 0.0), F0);
        
    vec3 numerator = NDF * G * F; 
    float denom = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
    vec3 specular = numerator / denom;
    
    vec3 kS = F;
    vec3 kD = vec3(1.0) - kS;
    kD *= 1.0 - metallic;      

    float NdotL = max(dot(N, L), 0.0);        

    vec3 Lo = (kD * albedo / PI + specular) * radiance * NdotL;
    
    vec3 ambient = vec3(0.03) * albedo;
    vec3 color = ambient + Lo;

    // --- ACCUMULATION & TONEMAPPING ---
    ivec2 coords = ivec2(gl_FragCoord.xy);
    
    if (pc.frame_index == 0) {
        imageStore(accum_buffer, coords, vec4(color, 1.0));
        outColor = vec4(color, 1.0);
    } else {
        vec3 prev = imageLoad(accum_buffer, coords).rgb;
        float weight = 1.0 / (float(pc.frame_index) + 1.0);
        vec3 blended = mix(prev, color, weight);
        imageStore(accum_buffer, coords, vec4(blended, 1.0));
        
        vec3 mapped = blended / (blended + vec3(1.0));
        mapped = pow(mapped, vec3(1.0/2.2)); 
        outColor = vec4(mapped, 1.0);
    }
    
    // MISE A JOUR PROFONDEUR (Crucial pour l'intersection 3D)
    gl_FragDepth = gl_FragCoord.z + (1.0 - sqrt(1.0 - mag)) * 0.0001;
}
#endif