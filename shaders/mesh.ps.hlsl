#include "mesh.root"
#include "defines.hlsl"

struct Constants {
    vec3 camera_position;
    vec3 camera_direction;

    vec3 light_direction;
    float light_radiance;

    uint albedo_index;
    // vec3 diffuse_color;

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

struct MeshConstants {
    mat4 transform;
    u32 albedo_index;
};

StructuredBuffer<MeshConstants> g_mesh_constants: register(t0, space0);
SamplerState linear_sampler: register(s0);
Texture2D<vec4> textures[]: register(t0, space1);

struct PS_INPUT
{
    float4 pos: SV_POSITION;

    float3 world_pos: POSITION;
    float3 normal: NORMAL;
    float2 uv: TEXCOORD;
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
    vec3 light_d = g_constants.light_direction;
    vec3 camera_p = g_constants.camera_position;
    // vec3 diffuse = g_constants.diffuse_color;

    vec3 specular = vec3(1, 1, 1);

    vec3 L = normalize(-light_d);
    vec3 N = normalize(input.normal);
    vec3 V = normalize(camera_p - input.world_pos);

    vec3 ka = 0.1;
    vec3 kd = max(dot(L, N), 0) * g_constants.light_radiance;

    /*
    vec3 H = normalize(L + V);

    vec3 ks = pow(max(dot(N, H), 0), 16.0) * 0.0;
    */
    // vec3 diff = IntToColor(g_draw_constants.index);

    vec3 albedo;
    if(g_mesh_constants[g_draw_constants.index].albedo_index != 0xFFFFFFFF) {
        albedo = textures[g_mesh_constants[g_draw_constants.index].albedo_index].Sample(linear_sampler, input.uv).rgb;
    } else {
        albedo = vec3(0.5, 0.1, 0.1);
    }

    vec3 color = albedo / PI * (ka + kd);// + specular * ks;

    return vec4(color, 1.0f);
}
