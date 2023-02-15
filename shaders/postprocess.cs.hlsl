#include "defines.hlsl"
#include "utils.hlsl"

#define MyRS1 \
    "DescriptorTable( " \
    "SRV( t0, offset=8 ), " \
    "UAV( u0, offset=9 ) " \
    ")," \
    "RootConstants(num32BitConstants=2, b0)"

Texture2D<vec4> input : register(t0);
RWTexture2D<vec4> output : register(u0);

struct ConstantBuf {
    u32 samples;
    u32 debug;
};

ConstantBuffer<ConstantBuf> g_constants: register(b0);

[RootSignature(MyRS1)]
[numthreads(1, 1, 1)]
void main(uint3 group_id : SV_GroupID) {
    vec4 value = input[group_id.xy];
    vec3 color = value.w > 0 ? value.rgb / value.w : 0.0;

    if(g_constants.debug == 0 || g_constants.debug == 5) {
        color = color / (color + 1.0);

        color.r = linearToSRGB(color.r);
        color.g = linearToSRGB(color.g);
        color.b = linearToSRGB(color.b);
    }

    output[group_id.xy].rgb = color;
}
