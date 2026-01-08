#version 460
layout(location = 0) in vec3 vColor;
layout(location = 1) in vec3 vNormal;
layout(location = 0) out vec4 outColor;

void main() {
    // On utilise les normales pour créer des couleurs (Visualisation du volume)
    vec3 N = normalize(vNormal);
    outColor = vec4(N * 0.5 + 0.5, 1.0);
}