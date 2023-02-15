#include "imgui.common.hlsl"


SamplerState sampler0 : register(s0);
Texture2D texture0 : register(t0);

[RootSignature(MyRS1)]
float4 main(PS_INPUT input) : SV_Target
{
    float4 out_col = input.col * texture0.Sample(sampler0, input.uv);
    return out_col;
}
