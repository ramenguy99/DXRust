#include "mesh.root"
#include "defines.hlsl"

struct Constants {
    vec3 camera_position;
    vec3 light_position;
    vec3 diffuse_color;

    mat4 projection;
    mat4 view;
    mat4 model;
    mat4 normal;
};

struct DrawConstants {
    uint index;
};

ConstantBuffer<Constants> g_constants: register(b0);
ConstantBuffer<DrawConstants> g_draw_constants: register(b1);

struct PS_INPUT
{
    float4 pos: SV_POSITION;

    float3 world_pos: POSITION;
    float3 normal: NORMAL;
};


uint MurmurMix(uint Hash)
{
	Hash ^= Hash >> 16;
	Hash *= 0x85ebca6b;
	Hash ^= Hash >> 13;
	Hash *= 0xc2b2ae35;
	Hash ^= Hash >> 16;
	return Hash;
}

float3 IntToColor(uint Index)
{
	uint Hash = MurmurMix(Index);

	float3 Color = float3
	(
		(Hash >>  0) & 255,
		(Hash >>  8) & 255,
		(Hash >> 16) & 255
	);

	return Color * (1.0f / 255.0f);
}

[RootSignature(MyRS1)]
float4 main(PS_INPUT input) : SV_Target
{
    vec3 light_p = g_constants.light_position;
    vec3 camera_p = g_constants.camera_position;
    vec3 diffuse = g_constants.diffuse_color;

    vec3 specular = vec3(1, 1, 1);

    vec3 L = normalize(light_p - input.world_pos);
    vec3 N = normalize(input.normal);
    vec3 V = normalize(camera_p - input.world_pos);
    vec3 H = normalize(L + V);

    vec3 ka = 0.1;
    vec3 kd = max(dot(L, N), 0);
    vec3 ks = pow(max(dot(N, H), 0), 16.0) * 0.0;
    
    vec3 diff = IntToColor(g_draw_constants.index);
    vec3 color = diff * (ka + kd) + specular * ks;

    return vec4(color, 1.0f);
}
