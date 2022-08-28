#include "defines.hlsl"

#define MyRS1 "DescriptorTable( UAV( u0 ) )" 

RWTexture2D<vec4> output : register(u0);

[RootSignature(MyRS1)]
[numthreads(1, 1, 1)]
void main(uint3 group_id : SV_GroupID, uint group_index : SV_GroupIndex) {
    output[group_id.xy] = vec4(0.5f, 1.0f, 0.5f, 1.0f);
}
    