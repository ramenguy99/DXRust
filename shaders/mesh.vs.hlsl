#include "mesh.root"
#include "defines.hlsl"

struct Constants {
    vec3 camera_position;

    vec3 light_position;

    vec3 diffuse_color;
    float film_dist;

    mat4 projection;
    mat4 view;
    mat4 model;
    mat4 normal;
};

ConstantBuffer<Constants> g_constants: register(b0);

struct VS_INPUT
{
    float3 pos : POSITION;
    float3 normal : NORMAL;
};

struct PS_INPUT
{
    float4 pos: SV_POSITION;

    float3 world_pos: POSITION;
    float3 normal: NORMAL;
};

[RootSignature(MyRS1)]
PS_INPUT main(VS_INPUT input)
{
#if 1
    vec4 world_pos = mul(g_constants.model, float4(input.pos, 1.0));

    PS_INPUT output;
    output.pos = mul(g_constants.projection, mul(g_constants.view, world_pos));
    output.world_pos = world_pos.xyz;
    output.normal = mul(g_constants.normal, float4(input.normal, 0.0)).xyz;

    return output;
#else
    vec4 world_pos = float4(input.pos, 1.0);

    PS_INPUT output;
    output.pos = world_pos;
    output.world_pos = world_pos;
    output.normal = input.normal;

    return output;
#endif
#
};
