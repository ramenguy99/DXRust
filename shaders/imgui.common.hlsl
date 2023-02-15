#define MyRS1 \
    "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT " \
       "| DENY_HULL_SHADER_ROOT_ACCESS" \
       "| DENY_DOMAIN_SHADER_ROOT_ACCESS" \
       "| DENY_GEOMETRY_SHADER_ROOT_ACCESS)," \
    "RootConstants(num32BitConstants=16, b0)," \
    "DescriptorTable(SRV(t0), visibility=SHADER_VISIBILITY_PIXEL),"\
    "StaticSampler(s0," \
        "filter = FILTER_MIN_MAG_MIP_LINEAR," \
        "maxAnisotropy = 0," \
        "comparisonFunc = COMPARISON_ALWAYS," \
        "borderColor = STATIC_BORDER_COLOR_TRANSPARENT_BLACK," \
        "maxLOD = 0," \
        "visibility = SHADER_VISIBILITY_PIXEL)"

struct VS_INPUT
{
    float2 pos : POSITION;
    float4 col : COLOR0;
    float2 uv  : TEXCOORD0;
};

struct PS_INPUT
{
    float4 pos : SV_POSITION;
    float4 col : COLOR0;
    float2 uv  : TEXCOORD0;
};
