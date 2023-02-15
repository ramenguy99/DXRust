#define MyRS1 \
    "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT " \
       "| DENY_HULL_SHADER_ROOT_ACCESS" \
       "| DENY_DOMAIN_SHADER_ROOT_ACCESS" \
       "| DENY_GEOMETRY_SHADER_ROOT_ACCESS)," \
    "CBV(b0),"\
    "RootConstants(num32BitConstants=1, b1),"\
    "SRV(t0),"\
    "DescriptorTable(SRV(t0, numDescriptors=unbounded, space=1, offset=10, flags=DESCRIPTORS_VOLATILE)),"\
    "StaticSampler(s0, filter = FILTER_MIN_MAG_MIP_LINEAR),"\

struct DrawConstants {
    uint index;
};

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
