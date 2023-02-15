#include "types.hlsl"
#include "mesh.common.hlsl"

ConstantBuffer<Constants> g_constants: register(b0);
ConstantBuffer<DrawConstants> g_draw_constants: register(b1);

StructuredBuffer<RasterMeshInstance> g_mesh_instances: register(t0, space0);

[RootSignature(MyRS1)]
PS_INPUT main(VS_INPUT input)
{
#if 1
    mat4 model = g_mesh_instances[g_draw_constants.index].transform;
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
