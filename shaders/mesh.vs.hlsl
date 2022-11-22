#include "mesh.root"
#include "defines.hlsl"

struct Constants {
    vec3 camera_position;

    vec3 camera_direction;

    vec3 light_position;

    vec3 diffuse_color;
    float film_dist;

    mat4 projection;
    mat4 view;
};

struct DrawConstants {
    uint index;
};

struct MeshConstants {
    mat4 transform;
    u32 albedo_index;
};

ConstantBuffer<Constants> g_constants: register(b0);
ConstantBuffer<DrawConstants> g_draw_constants: register(b1);

StructuredBuffer<MeshConstants> g_mesh_constants: register(t0, space0);

struct VS_INPUT
{
    float3 pos : POSITION;
    float3 normal : NORMAL;
    float2 uv: TEXCOORD;
};

struct PS_INPUT
{
    float4 pos: SV_POSITION;

    float3 world_pos: POSITION;
    float3 normal: NORMAL;
    float2 uv: TEXCOORD;
};

[RootSignature(MyRS1)]
PS_INPUT main(VS_INPUT input)
{
#if 1
    mat4 model = g_mesh_constants[g_draw_constants.index].transform;
    vec4 world_pos = mul(model, float4(input.pos, 1.0));

    PS_INPUT output;
    output.pos = mul(g_constants.projection, mul(g_constants.view, world_pos));
    output.world_pos = world_pos.xyz;
    output.normal = mul(model, float4(input.normal, 0.0)).xyz;
    output.uv = input.uv;

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
