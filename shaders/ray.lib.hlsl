#include "defines.hlsl"
#include "random.hlsl"
#include "types.hlsl"
#include "utils.hlsl"
#include "principled_brdf.hlsl"

GlobalRootSignature MyGlobalRootSignature =
{
    "DescriptorTable("
        "UAV(u0, offset=1)," // 1 - output
        "SRV(t0), "          // 2 - acceleration structure
        "SRV(t1), "          // 3 - index buffer
        "SRV(t2), "          // 4 - normals
        "SRV(t3), "          // 5 - tangents
        "SRV(t4), "          // 6 - uvs
        "SRV(t5), "          // 7 - instances data
                             // 8 - postprocess input
                             // 9 - postporcess output
        "SRV(t6, offset=10, numDescriptors=unbounded, flags=DESCRIPTORS_VOLATILE)" // 10 - Textures
    "),"
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
Buffer<vec3> tangents_buffer: register(t3);
Buffer<vec2> uvs_buffer: register(t4);

StructuredBuffer<RayMeshInstance> instances_buffer: register(t5);
Texture2D<vec4> textures[]: register(t6);
SamplerState linear_sampler: register(s0);


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

    float2 jitter = rand2(seed);

    vec3 dir;
    vec3 origin;
    GenerateCameraRay(p, jitter, origin, dir);

    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = dir;
    ray.TMin = 0.01;
    ray.TMax = 100000.0;

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

    vec4 color = vec4(payload.color, 1.0);
    // if(any(isnan(payload.color))) {
    //     color = 0.0;
    // }

    if(g_constants.samples == 1) {
        output[p].xyzw = color;
    } else {
        output[p].xyzw += color;
    }
}


[shader("miss")]
void Miss(inout HitInfo payload)
{
    payload.distance = -1.0;
}

[shader("closesthit")]
void ClosestHit(inout HitInfo payload, in BuiltInTriangleIntersectionAttributes attribs)
{
    // Hit position
    vec3 direction = WorldRayDirection();
    vec3 position = WorldRayOrigin() + RayTCurrent() * direction;

    // Mesh info
    uint mesh_index = InstanceID();
    RayMeshInstance instance = instances_buffer[mesh_index];

    // Primitive info
    uint triangle_index = PrimitiveIndex();
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
    vec3 N = normalize(mul((float3x3)ObjectToWorld(), normal));

    vec2 uv =
        uvs_buffer[indices.x + vertex_offset] * (1 - barycentrics.x - barycentrics.y) +
        uvs_buffer[indices.y + vertex_offset] * barycentrics.x +
        uvs_buffer[indices.z + vertex_offset] * barycentrics.y;


    // Material info
    vec3 albedo = 1.0;
    if(instance.albedo_index != 0xFFFFFFFF) {
       albedo = textures[instance.albedo_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        albedo = instance.albedo_value.rgb;
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

    // Local frame
    Frame frame;
    if(all(payload.throughput == 1.0)) {
        vec3 tangent =
            tangents_buffer[indices.x + vertex_offset] * (1 - barycentrics.x - barycentrics.y) +
            tangents_buffer[indices.y + vertex_offset] * barycentrics.x +
            tangents_buffer[indices.z + vertex_offset] * barycentrics.y;
        vec3 T = normalize(mul((float3x3)ObjectToWorld(), tangent));
        vec3 B = normalize(cross(N, T));
        if (any(isnan(B)) || g_constants.debug == 5) {
            frame = frameFromNormal(N);
        }
        else {
            frame = makeFrame(T, B, N);
            if(instance.normal_index != 0xFFFFFFFF) {
                vec2 n = textures[instance.normal_index].SampleLevel(linear_sampler, uv, 0.0f).rg * 2.0 - 1.0;
                // Lerp towards local +Z when viewing at grazing angle
                float weight = max(dot(N, -direction), 0.0);
                n = lerp(0.0, n, weight);

                float z = sqrt(1.0 - n.x * n.x - n.y * n.y);
                N = toWorld(frame, vec3(n, z));
                B = normalize(cross(N, T));
                T = normalize(cross(B, N));
                frame = makeFrame(T, B, N);
            }
        }
    } else {
        frame = frameFromNormal(N);
    }

    vec3 wo = toLocal(frame, -direction);

    // BRDF
    float roughness = max(specular.r, 0.05);
    float metallic = specular.g;
    float alpha = square(roughness);

    // Direct lighting
    vec3 radiance = g_constants.light_radiance;
    vec3 L = -g_constants.light_direction;

    // Shadowing
    RayDesc shadow_ray;
    shadow_ray.Origin = position;
    shadow_ray.Direction = L;
    shadow_ray.TMin = 0.01;
    shadow_ray.TMax = 100000.0;
    HitInfo shadow_payload = {
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        0.0,
        uvec4(0, 0, 0, 0),
    };
    TraceRay(scene,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH |
        RAY_FLAG_SKIP_CLOSEST_HIT_SHADER, ~0, 0, 1, 0,
        shadow_ray, shadow_payload);

    if(shadow_payload.distance < 0.0) {
        vec3 wi = toLocal(frame, L);
        vec3 f = evalPrincipledBrdf(alpha, metallic, albedo, wo, wi);
        payload.color += payload.throughput * f * radiance * max(dot(N, L), 0);
    }


    // Brdf sampling
    vec3 u = rand3(payload.seed);

    vec3 sampled_dir;
    float pdf;
    vec3 f = samplePrincipledBrdf(alpha, metallic, albedo, wo, u, sampled_dir, pdf);

    payload.direction = toWorld(frame, sampled_dir);
    payload.throughput *= pdf > 0.0 ? f / pdf : 0.0;
    payload.distance = RayTCurrent();

    // Debug
    switch(g_constants.debug) {
        case 1: payload.color = roughness; break;
        case 2: payload.color = metallic; break;
        case 3: payload.color = emissive; break;
        case 4: payload.color = N * 0.5 + 0.5; break;
        case 5: break;
    }
}
