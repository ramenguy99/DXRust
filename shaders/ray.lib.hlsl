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
    56, // max payload size
    8   // max attribute size
};

RaytracingPipelineConfig MyPipelineConfig =
{
    2 // max trace recursion depth
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
    uint normal_index;
    uint specular_index;
    uint emissive_index;

    vec4 albedo_value;
    vec4 specular_value;
    vec4 emissive_value;
};

StructuredBuffer<MeshInstance> instances_buffer: register(t4);
Texture2D<vec4> textures[]: register(t0, space1);
SamplerState linear_sampler: register(s0);

struct Constants {
    vec3 camera_position;

    vec3 camera_direction;

    vec3 light_direction;
    float light_radiance;

    vec3 diffuse_color;
    float film_dist;

    mat4 projection;
    mat4 view;

    u32 frame_index;
    u32 samples;
    float emissive_multiplier;
    u32 debug;
};

ConstantBuffer<Constants> g_constants: register(b0);

struct HitInfo
{
    vec3 color;
    vec3 throughput;
    vec3 direction;
    float distance;
    uvec4 seed;
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

    HitInfo payload = {
        vec3(0, 0, 0),
        vec3(1, 1, 1),
        vec3(0, 0, 0),
        0.0,
        seed,
    };

    for(int i = 0; i < 8; i++) {
        TraceRay(scene, RAY_FLAG_CULL_BACK_FACING_TRIANGLES, ~0, 0, 1, 0, ray, payload);

        if(payload.distance < 0 || all(payload.throughput <= 0.001)) {
            break;
        } else {
            ray.Origin += payload.distance * ray.Direction;
            ray.Direction = payload.direction;
        }

        if(g_constants.debug) {
            break;
        }
    }

    vec3 old_color = output[p].xyz;
    float samples = g_constants.samples;
    output[p].xyz = (old_color * (samples - 1) + payload.color) / samples;
    output[p].w = 1.0;
}


[shader("miss")]
void Miss(inout HitInfo payload)
{
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

mat3 frameFromDirection(vec3 a) {
    vec3 b, c;
    if (abs(a.x) > abs(a.y)) {
        float invLen = 1.0f / sqrt(a.x * a.x + a.z * a.z);
        c = vec3(a.z * invLen, 0.0f, -a.x * invLen);
    } else {
        float invLen = 1.0f / sqrt(a.y * a.y + a.z * a.z);
        c = vec3(0.0f, a.z * invLen, -a.y * invLen);
    }
    b = cross(c, a);
    return mat3(b, c, a);
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


    vec3 camera_p = g_constants.camera_position;
    vec3 diffuse = g_constants.diffuse_color;

    vec3 L = -g_constants.light_direction;

    RayDesc shadow_ray;
    shadow_ray.Origin = position;
    shadow_ray.Direction = L;
    shadow_ray.TMin = 0.01;
    shadow_ray.TMax = 1000.0;
    HitInfo shadow_payload = {
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        0.0,
        uvec4(0, 0, 0, 0),
    };

    TraceRay(scene,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH |
        RAY_FLAG_CULL_BACK_FACING_TRIANGLES |
        RAY_FLAG_SKIP_CLOSEST_HIT_SHADER, ~0, 0, 1, 0,
        shadow_ray, shadow_payload);

    vec3 N = normalize(mul((float3x3)ObjectToWorld(), normal));
    vec2 uv =
        uvs_buffer[indices.x + vertex_offset] * (1 - barycentrics.x - barycentrics.y) +
        uvs_buffer[indices.y + vertex_offset] * barycentrics.x +
        uvs_buffer[indices.z + vertex_offset] * barycentrics.y;

    vec3 albedo = 1.0;

    if(instance.albedo_index != 0xFFFFFFFF) {
       albedo = textures[instance.albedo_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        albedo = instance.albedo_value.rgb;
    }

    vec3 radiance = g_constants.light_radiance;

    if(shadow_payload.distance < 0.0) {
        payload.color += payload.throughput * max(dot(N, L), 0) * radiance * albedo / PI;
    }

    vec3 emissive = 0;
    if(instance.emissive_index != 0xFFFFFFFF) {
        emissive = textures[instance.emissive_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        emissive = instance.emissive_value.rgb;
    }
    payload.color += payload.throughput * emissive * g_constants.emissive_multiplier;

    vec2 specular = 0;
    if(instance.specular_index != 0xFFFFFFFF) {
        specular = textures[instance.specular_index].SampleLevel(linear_sampler, uv, 0.0f).gb;
    } else {
        specular = instance.specular_value.gb;
    }

    float roughness = specular.r;
    float metallic = specular.g;

    switch(g_constants.debug) {
        case 1: payload.color = roughness; break;
        case 2: payload.color = metallic; break;
        case 3: payload.color = emissive; break;
        case 4: payload.color = N * 0.5 + 1.0; break;
    }
        /*
        vec3 V = normalize(camera_p - position);
        vec3 H = normalize(L + V);

        vec3 ka = 0.1;
        vec3 kd = max(dot(L, N), 0);
        vec3 ks = pow(max(dot(N, H), 0), 16.0) * 0.0;

        // vec3 color = diffuse * (ka + kd) + specular * ks;
        // vec3 color = specular * (PrimitiveIndex() / 100000.0);
        // vec3 color = N * 0.5 + 1.0;
        // vec3 color = vec3(barycentrics, 1- barycentrics.x - barycentrics.y);
        vec3 diff = 1.0;
*/
/*
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
*/
    mat3 frame = frameFromDirection(N);
    payload.direction = mul(frame, sampleCosineWeightedHemisphere(payload.seed));
    payload.throughput *= albedo;
    payload.distance = RayTCurrent();
}
