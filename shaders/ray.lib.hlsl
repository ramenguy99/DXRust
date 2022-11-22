#include "defines.hlsl"
#include "random.hlsl"

GlobalRootSignature MyGlobalRootSignature =
{
    "DescriptorTable(UAV(u0, offset=1), SRV(t0), SRV(t1), SRV(t2), SRV(t3), SRV(t4), SRV(t0, numDescriptors=unbounded, space=1, flags=DESCRIPTORS_VOLATILE))," // output, as, indices, normals, uvs, instances data
    "CBV(b0)," // constants
    "StaticSampler(s0, filter = FILTER_MIN_MAG_MIP_LINEAR),"
};

TriangleHitGroup HitGroup =
{
    "",             // AnyHit
    "ClosestHit",   // ClosestHit
};

RaytracingShaderConfig  MyShaderConfig =
{
    16, // max payload size
    8   // max attribute size
};

RaytracingPipelineConfig MyPipelineConfig =
{
    1 // max trace recursion depth
};


RWTexture2D<vec4> output : register(u0);
RaytracingAccelerationStructure scene : register(t0);

// Geometry data for first instance
Buffer<uint> index_buffer: register(t1);
Buffer<vec3> normals_buffer: register(t2);
Buffer<vec2> uvs_buffer: register(t3);

struct MeshInstance {
    uint vertex_offset;
    uint index_offset;
    uint albedo_index;
};

StructuredBuffer<MeshInstance> instances_buffer: register(t4);
Texture2D<vec4> textures[]: register(t0, space1);
SamplerState linear_sampler: register(s0);

struct Constants {
    vec3 camera_position;

    vec3 camera_direction;

    vec3 light_position;

    vec3 diffuse_color;
    float film_dist;

    mat4 projection;
    mat4 view;

    u32 frame_index;
    u32 samples;
};

ConstantBuffer<Constants> g_constants: register(b0);

struct HitInfo
{
    vec3 color;
    float distance;
};

inline void GenerateCameraRay(uint2 index, float2 jitter, out float3 origin, out float3 direction)
{
    float2 xy = index + 0.5;
    float2 offset = xy / DispatchRaysDimensions().xy * 2.0 - 1.0;
    jitter = (jitter * 2.0f) / DispatchRaysDimensions().xy;
    offset += jitter;

    vec3 camera_forward = g_constants.camera_direction;
    vec3 world_up = vec3(0, 0, 1);
    vec3 camera_right = normalize(cross(camera_forward, world_up));
    vec3 camera_up = cross(camera_right, camera_forward);
    vec3 camera_p = g_constants.camera_position;

    float film_dist = g_constants.film_dist;
    vec2 film_size = vec2(1.0, 1.0);
    film_size.y = (float)DispatchRaysDimensions().y / (float)DispatchRaysDimensions().x;
    vec2 half_film = film_size * 0.5f;
    vec3 film_center = camera_p - film_dist * camera_forward;

    origin = film_center + offset.x * half_film.x * camera_right
                         + offset.y * half_film.y * camera_up;

    direction = normalize(camera_p - origin);
}

[shader("raygeneration")]
void RayGeneration()
{
    uvec2 p = DispatchRaysIndex().xy;
    uvec4 seed = uvec4(p, uint(g_constants.frame_index), uint(p.x) + uint(p.y));
    rand(seed);
    float2 jitter = rand2(seed);

    vec3 dir;
    vec3 origin;
    GenerateCameraRay(p, jitter, origin, dir);

    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = dir;
    ray.TMin = 0.01;
    ray.TMax = 1000.0;

    HitInfo payload = { vec3(0, 0, 0), 0.0 };
    TraceRay(scene, RAY_FLAG_CULL_BACK_FACING_TRIANGLES, ~0, 0, 1, 0, ray, payload);

    vec3 old_color = output[p].xyz;
    float samples = g_constants.samples;
    output[p].xyz = (old_color * (samples - 1) + payload.color) / samples;
    output[p].w = 1.0;
}


[shader("miss")]
void Miss(inout HitInfo payload)
{
    payload.color = vec3(0.1, 0.1, 0.1);
    payload.distance = -1.0;
}


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

[shader("closesthit")]
void ClosestHit(inout HitInfo payload, in BuiltInTriangleIntersectionAttributes attribs)
{
    vec3 position = WorldRayOrigin() + RayTCurrent() * WorldRayDirection();

    uint triangle_index = PrimitiveIndex();
    uint mesh_index = InstanceID();
    MeshInstance instance = instances_buffer[mesh_index];
    uint index_offset = instance.index_offset;
    uint vertex_offset = instance.vertex_offset;

    uint3 indices;
    indices.x = index_buffer[triangle_index * 3 + 0 + index_offset];
    indices.y = index_buffer[triangle_index * 3 + 1 + index_offset];
    indices.z = index_buffer[triangle_index * 3 + 2 + index_offset];

    vec2 barycentrics = attribs.barycentrics;

    vec3 normal =
        normals_buffer[indices.x + vertex_offset] * (1 - barycentrics.x - barycentrics.y) +
        normals_buffer[indices.y + vertex_offset] * barycentrics.x +
        normals_buffer[indices.z + vertex_offset] * barycentrics.y;


    vec3 light_p = g_constants.light_position;
    vec3 camera_p = g_constants.camera_position;
    vec3 diffuse = g_constants.diffuse_color;

    vec3 specular = vec3(1, 1, 1);

    vec3 L = normalize(light_p - position);
    vec3 N = normalize(mul((float3x3)ObjectToWorld(), normal));
    vec3 V = normalize(camera_p - position);
    vec3 H = normalize(L + V);

    vec3 ka = 0.1;
    vec3 kd = max(dot(L, N), 0);
    vec3 ks = pow(max(dot(N, H), 0), 16.0) * 0.0;

    // vec3 color = diffuse * (ka + kd) + specular * ks;
    // vec3 color = specular * (PrimitiveIndex() / 100000.0);
    // vec3 color = N * 0.5 + 1.0;
    // vec3 color = vec3(barycentrics, 1- barycentrics.x - barycentrics.y);
    // vec3 diff = IntToColor(mesh_index);

    vec3 diff;
    if(instance.albedo_index != 0xFFFFFFFF) {
        vec2 uv =
            uvs_buffer[indices.x + vertex_offset] * (1 - barycentrics.x - barycentrics.y) +
            uvs_buffer[indices.y + vertex_offset] * barycentrics.x +
            uvs_buffer[indices.z + vertex_offset] * barycentrics.y;

       diff = textures[instance.albedo_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        diff = vec3(0.5, 0.1, 0.1);
    }

    vec3 color = diff * (ka + kd) + specular * ks;
    payload.color = color;

    payload.distance = RayTCurrent();
}
