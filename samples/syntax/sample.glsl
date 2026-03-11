// GLSL Syntax Highlighting Test
// A deferred rendering pipeline with PBR lighting and post-processing.

#version 450 core
#extension GL_ARB_separate_shader_objects : enable

// ============================================================
// Shared uniforms
// ============================================================

layout(std140, binding = 0) uniform CameraBlock {
    mat4 view;
    mat4 projection;
    mat4 viewProjection;
    mat4 inverseViewProjection;
    vec3 cameraPosition;
    float nearPlane;
    float farPlane;
    float time;
    vec2 resolution;
} camera;

layout(std140, binding = 1) uniform LightBlock {
    vec3 direction;
    float intensity;
    vec3 color;
    float ambientIntensity;
    mat4 shadowMatrix;
    vec2 shadowMapSize;
    float shadowBias;
    float _pad;
} sun;

#define MAX_POINT_LIGHTS 64
#define PI 3.14159265359
#define INV_PI 0.31830988618
#define EPSILON 1e-6

struct PointLight {
    vec3 position;
    float radius;
    vec3 color;
    float intensity;
};

layout(std430, binding = 2) buffer PointLightBuffer {
    int lightCount;
    PointLight lights[MAX_POINT_LIGHTS];
};

// ============================================================
// G-Buffer pass - Vertex Shader
// ============================================================

#ifdef VERTEX_SHADER

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_texcoord;
layout(location = 3) in vec4 a_tangent;

layout(location = 0) out vec3 v_worldPos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_texcoord;
layout(location = 3) out mat3 v_TBN;

uniform mat4 u_model;
uniform mat3 u_normalMatrix;

void main() {
    vec4 worldPos = u_model * vec4(a_position, 1.0);
    v_worldPos = worldPos.xyz;
    v_normal = normalize(u_normalMatrix * a_normal);
    v_texcoord = a_texcoord;

    // Construct TBN matrix for normal mapping
    vec3 T = normalize(u_normalMatrix * a_tangent.xyz);
    vec3 N = v_normal;
    T = normalize(T - dot(T, N) * N); // Gram-Schmidt re-orthogonalize
    vec3 B = cross(N, T) * a_tangent.w;
    v_TBN = mat3(T, B, N);

    gl_Position = camera.viewProjection * worldPos;
}

#endif

// ============================================================
// G-Buffer pass - Fragment Shader
// ============================================================

#ifdef FRAGMENT_SHADER_GBUFFER

layout(location = 0) in vec3 v_worldPos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_texcoord;
layout(location = 3) in mat3 v_TBN;

// G-Buffer outputs
layout(location = 0) out vec4 gAlbedo;     // RGB: albedo, A: metallic
layout(location = 1) out vec4 gNormal;     // RGB: world normal (encoded), A: roughness
layout(location = 2) out vec4 gEmission;   // RGB: emission, A: AO

// Material textures
layout(binding = 0) uniform sampler2D u_albedoMap;
layout(binding = 1) uniform sampler2D u_normalMap;
layout(binding = 2) uniform sampler2D u_metallicRoughnessMap;
layout(binding = 3) uniform sampler2D u_emissionMap;
layout(binding = 4) uniform sampler2D u_aoMap;

uniform vec4 u_albedoFactor;
uniform float u_metallicFactor;
uniform float u_roughnessFactor;
uniform vec3 u_emissionFactor;

// Encode normal to octahedral representation
vec2 encodeNormal(vec3 n) {
    n /= (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z < 0.0) {
        n.xy = (1.0 - abs(n.yx)) * vec2(
            n.x >= 0.0 ? 1.0 : -1.0,
            n.y >= 0.0 ? 1.0 : -1.0
        );
    }
    return n.xy * 0.5 + 0.5;
}

void main() {
    vec4 albedo = texture(u_albedoMap, v_texcoord) * u_albedoFactor;

    // Alpha test
    if (albedo.a < 0.5) discard;

    // Normal mapping
    vec3 normalTS = texture(u_normalMap, v_texcoord).rgb * 2.0 - 1.0;
    vec3 normal = normalize(v_TBN * normalTS);

    // Metallic-roughness
    vec2 mr = texture(u_metallicRoughnessMap, v_texcoord).bg;
    float metallic = mr.x * u_metallicFactor;
    float roughness = mr.y * u_roughnessFactor;

    // Emission and AO
    vec3 emission = texture(u_emissionMap, v_texcoord).rgb * u_emissionFactor;
    float ao = texture(u_aoMap, v_texcoord).r;

    // Pack G-Buffer
    gAlbedo = vec4(albedo.rgb, metallic);
    gNormal = vec4(encodeNormal(normal), roughness, 1.0);
    gEmission = vec4(emission, ao);
}

#endif

// ============================================================
// Lighting pass - PBR functions
// ============================================================

#ifdef FRAGMENT_SHADER_LIGHTING

// GGX/Trowbridge-Reitz normal distribution
float distributionGGX(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;

    float denom = NdotH2 * (a2 - 1.0) + 1.0;
    denom = PI * denom * denom;

    return a2 / max(denom, EPSILON);
}

// Schlick-GGX geometry function
float geometrySchlickGGX(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

float geometrySmith(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    return geometrySchlickGGX(NdotV, roughness) * geometrySchlickGGX(NdotL, roughness);
}

// Fresnel-Schlick
vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 fresnelSchlickRoughness(float cosTheta, vec3 F0, float roughness) {
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) *
           pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Cook-Torrance BRDF
vec3 cookTorranceBRDF(vec3 N, vec3 V, vec3 L, vec3 albedo,
                       float metallic, float roughness) {
    vec3 H = normalize(V + L);

    vec3 F0 = mix(vec3(0.04), albedo, metallic);

    float D = distributionGGX(N, H, roughness);
    float G = geometrySmith(N, V, L, roughness);
    vec3 F = fresnelSchlick(max(dot(H, V), 0.0), F0);

    // Specular
    vec3 numerator = D * G * F;
    float denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + EPSILON;
    vec3 specular = numerator / denominator;

    // Energy conservation
    vec3 kS = F;
    vec3 kD = (1.0 - kS) * (1.0 - metallic);

    float NdotL = max(dot(N, L), 0.0);

    return (kD * albedo * INV_PI + specular) * NdotL;
}

// G-Buffer samplers
layout(binding = 0) uniform sampler2D g_albedo;
layout(binding = 1) uniform sampler2D g_normal;
layout(binding = 2) uniform sampler2D g_emission;
layout(binding = 3) uniform sampler2D g_depth;
layout(binding = 4) uniform sampler2DShadow g_shadowMap;

in vec2 v_texcoord;
layout(location = 0) out vec4 fragColor;

// Decode octahedral normal
vec3 decodeNormal(vec2 encoded) {
    vec2 f = encoded * 2.0 - 1.0;
    vec3 n = vec3(f, 1.0 - abs(f.x) - abs(f.y));
    float t = clamp(-n.z, 0.0, 1.0);
    n.xy += vec2(
        n.x >= 0.0 ? -t : t,
        n.y >= 0.0 ? -t : t
    );
    return normalize(n);
}

// Reconstruct world position from depth
vec3 worldPosFromDepth(vec2 uv, float depth) {
    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
    vec4 worldPos = camera.inverseViewProjection * clipPos;
    return worldPos.xyz / worldPos.w;
}

void main() {
    // Sample G-Buffer
    vec4 albedoMetal = texture(g_albedo, v_texcoord);
    vec4 normalRough = texture(g_normal, v_texcoord);
    vec4 emissionAO = texture(g_emission, v_texcoord);
    float depth = texture(g_depth, v_texcoord).r;

    // Early out for sky
    if (depth >= 1.0) {
        fragColor = vec4(0.0);
        return;
    }

    // Unpack
    vec3 albedo = albedoMetal.rgb;
    float metallic = albedoMetal.a;
    vec3 N = decodeNormal(normalRough.rg);
    float roughness = normalRough.b;
    vec3 emission = emissionAO.rgb;
    float ao = emissionAO.a;

    vec3 worldPos = worldPosFromDepth(v_texcoord, depth);
    vec3 V = normalize(camera.cameraPosition - worldPos);

    // Directional light (sun)
    vec3 Lo = cookTorranceBRDF(N, V, -sun.direction, albedo, metallic, roughness)
              * sun.color * sun.intensity;

    // Point lights
    for (int i = 0; i < lightCount && i < MAX_POINT_LIGHTS; i++) {
        PointLight light = lights[i];
        vec3 L = light.position - worldPos;
        float dist = length(L);

        if (dist > light.radius) continue;

        L /= dist;
        float attenuation = 1.0 / (dist * dist + 1.0);
        attenuation *= smoothstep(light.radius, light.radius * 0.75, dist);

        Lo += cookTorranceBRDF(N, V, L, albedo, metallic, roughness)
              * light.color * light.intensity * attenuation;
    }

    // Ambient
    vec3 ambient = sun.ambientIntensity * albedo * ao;

    vec3 color = ambient + Lo + emission;

    fragColor = vec4(color, 1.0);
}

#endif
