#include "defines.hlsl"
#include "types.hlsl"
#include "mesh.common.hlsl"

ConstantBuffer<Constants> g_constants: register(b0);
ConstantBuffer<DrawConstants> g_draw_constants: register(b1);

StructuredBuffer<RasterMeshInstance> g_mesh_instances: register(t0, space0);
SamplerState linear_sampler: register(s0);
Texture2D<vec4> textures[]: register(t0, space1);

[RootSignature(MyRS1)]
float4 main(PS_INPUT input) : SV_Target
{
    vec3 light_d = g_constants.light_direction;
    vec3 camera_p = g_constants.camera_position;

    vec3 L = normalize(-light_d);
    vec3 N = normalize(input.normal);
    vec3 V = normalize(camera_p - input.world_pos);

    vec3 ka = 0.1;
    vec3 kd = max(dot(L, N), 0) * g_constants.light_radiance;

    RasterMeshInstance instance = g_mesh_instances[g_draw_constants.index];
    vec2 uv = input.uv;
    vec3 albedo;
    if(instance.albedo_index != 0xFFFFFFFF) {
        albedo = textures[instance.albedo_index].Sample(linear_sampler, uv).rgb;
    } else {
        albedo = instance.albedo_value;
    }

    vec3 emissive = 0;
    if(instance.emissive_index != 0xFFFFFFFF) {
        emissive = textures[instance.emissive_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        emissive = instance.emissive_value.rgb;
    }

    vec2 specular = 0;
    if(instance.specular_index != 0xFFFFFFFF) {
        specular = textures[instance.specular_index].SampleLevel(linear_sampler, uv, 0.0f).gb;
    } else {
        specular = instance.specular_value.gb;
    }

    float roughness = specular.r;
    float metallic = specular.g;

    vec3 color = albedo / PI * (ka + kd);// + emissive;

    switch(g_constants.debug) {
        case 1: color = roughness; break;
        case 2: color = metallic; break;
        case 3: color = emissive; break;
        case 4: color = N * 0.5 + 0.5; break;
    }

    return vec4(color, 1.0f);
}
