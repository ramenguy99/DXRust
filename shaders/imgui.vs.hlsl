#include "imgui.common.hlsl"

cbuffer vertexBuffer : register(b0)
{
    float4x4 ProjectionMatrix;
};

[RootSignature(MyRS1)]
PS_INPUT main(VS_INPUT input)
{
    PS_INPUT output;
    output.pos = mul(ProjectionMatrix, float4(input.pos.xy, 0.f, 1.f));
    output.col = input.col;
    output.uv  = input.uv;
    return output;
};
