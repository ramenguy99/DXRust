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
        "SRV(t0, space=1, offset=10, numDescriptors=unbounded, flags=DESCRIPTORS_VOLATILE)" // 10 - Textures
    "),"
    "CBV(b0)," // 1 - constants
    "SRV(t6)," // 2 - Lights buffer
    "SRV(t7)," // 3 - Lights cdf buffer
    "SRV(t8)," // 4 - Alias table buffer
    "StaticSampler(s0, filter = FILTER_MIN_MAG_MIP_LINEAR),"
};

TriangleHitGroup HitGroup =
{
    "",             // AnyHit
    "ClosestHit",   // ClosestHit
};

RaytracingShaderConfig  MyShaderConfig =
{
    64, // max payload size
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
Texture2D<vec4> textures[]: register(t0, space1);
StructuredBuffer<Light> lights_buffer: register(t6);
StructuredBuffer<float> lights_cdf_buffer: register(t7);
StructuredBuffer<Alias> alias_table: register(t8);

SamplerState linear_sampler: register(s0);
ConstantBuffer<Constants> g_constants: register(b0);

struct HitInfo
{
    vec3 color;
    vec3 throughput;
    vec3 direction;
    float distance;
    uvec4 seed;
    u32 bounce;
    float brdf_pdf;
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

#define LIGHT_SAMPLING 0
#define BRDF_SAMPLING 1
#define MIS 2
#define RIS 3

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
    ray.TMin = 0.001;
    ray.TMax = 100000.0;

    HitInfo payload = {
        vec3(0, 0, 0),
        vec3(1, 1, 1),
        vec3(0, 0, 0),
        0.0,
        seed,
        0,
        0.0,
    };

    uint max_bounces = g_constants.bounces;
    if(g_constants.sampling_mode != LIGHT_SAMPLING) {
        max_bounces += 1;
    }

    for(; payload.bounce < max_bounces; payload.bounce++) {
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
    if(any(isnan(payload.color))) {
        color = 0.0;
    }

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


struct LightSample {
    vec3 radiance;
    vec3 direction;        // From shaded point to light
    float sample_weight;
    float shadow_distance; // Max distance to use for shadow ray
    bool delta_light;
};

void sampleSun(inout LightSample s) {
    s.direction = -g_constants.light_direction;
    s.radiance = g_constants.light_radiance;
    s.shadow_distance = 100000.0;
    s.sample_weight = 1.0;
    s.delta_light = true;
}

void sampleAreaLights(inout LightSample s, vec3 position, vec4 u) {
    int l = 0;
    if (!g_constants.use_alias_table) {
        // Binary search for the smallest v with v > u.y
        int r = g_constants.num_lights - 1;
        while(l < r) {
            int mid = (r + l) / 2;
            if(lights_cdf_buffer[mid] < u.y) {
                l = mid + 1;
            } else {
                r = mid;
            }
        }
    } else {
        // Lookup into alias table
        uint i = clamp((uint)(u.x * g_constants.num_lights), 0, g_constants.num_lights - 1);
        if(u.y <= alias_table[i].p) {
            l = i;
        } else {
            l = alias_table[i].a;
        }
    }
    Light light = lights_buffer[l];

    float pdf = luminance(light.emissive) * g_constants.lights_pdf_normalization;

    vec2 tri_uv = sampleTriangle(u.zw);
    vec3 e1 = light.p1 - light.p0;
    vec3 e2 = light.p2 - light.p0;
    vec3 p = e1 * tri_uv.x + e2 * tri_uv.y + light.p0;
    vec3 n = normalize(cross(e2, e1));
    vec3 radiance = light.emissive * g_constants.emissive_multiplier;

    vec3 v = p - position;
    float dist2 = dot(v, v);
    float d = sqrt(dist2);

    s.direction = v / d;
    s.radiance = radiance;
    s.sample_weight = max(dot(-s.direction, n), 0.0f) / (pdf * dist2);
    s.shadow_distance = d - 1.0e-3;
    s.delta_light = false;
}

[shader("closesthit")]
void ClosestHit(inout HitInfo payload, in BuiltInTriangleIntersectionAttributes attribs)
{
    // Hit position
    vec3 direction = WorldRayDirection();
    float distance = RayTCurrent();
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
        // emissive = textures[instance.emissive_index].SampleLevel(linear_sampler, uv, 0.0f).rgb;
    } else {
        emissive = instance.emissive_value.rgb;
    }

    vec2 specular = 0;
    if(instance.specular_index != 0xFFFFFFFF) {
        specular = textures[instance.specular_index].SampleLevel(linear_sampler, uv, 0.0f).gb;
    } else {
        specular = instance.specular_value.gb;
    }


    const float SUN_P = clamp(g_constants.light_radiance * 10.0 /
    (g_constants.light_radiance * 10.0 + g_constants.emissive_multiplier), 0.05, 0.95);

    // Light hit
    if(any(emissive > 0.0)) {
        if(g_constants.sampling_mode == BRDF_SAMPLING || payload.bounce == 0) {
            payload.color += payload.throughput * emissive * g_constants.emissive_multiplier;
        } else if(g_constants.sampling_mode == MIS) {
            float brdf_pdf = payload.brdf_pdf;

            float area_pdf = (1.0 - SUN_P) * luminance(emissive) * g_constants.lights_pdf_normalization;
            float light_pdf = (area_pdf * square(distance)) / dot(-direction, N);

            if(light_pdf > 0.0) {
                float mis_weight = brdf_pdf * balance_heuristic(brdf_pdf, light_pdf);
                payload.color += mis_weight * payload.throughput * emissive * g_constants.emissive_multiplier;
            }
        }
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

    // BRDF
    float roughness = max(specular.r, 0.05);
    float metallic = specular.g;
    float alpha = square(roughness);
    vec3 wo = toLocal(frame, -direction);

    vec4 u = rand4(payload.seed);
    LightSample light_sample;
    if(g_constants.sampling_mode == BRDF_SAMPLING || u.x < SUN_P) {
        sampleSun(light_sample);
        light_sample.sample_weight *= g_constants.sampling_mode == BRDF_SAMPLING ? 1.0 : 1.0 / SUN_P;
    } else {
        u.x = (u.x - SUN_P) / (1.0 - SUN_P);
        if(g_constants.sampling_mode == RIS) {
            vec4 y = 0;
            float w_sum = 0;
            uint M = max(g_constants.ris_count / ((payload.bounce + 1) * (payload.bounce + 1)), 1);

            for(int i = 0; i < M; i++) {
                sampleAreaLights(light_sample, position, u);
                vec3 wi = toLocal(frame, light_sample.direction);
                vec3 f = evalPrincipledBrdf(alpha, metallic, albedo, wo, wi);
                float target_pdf = luminance(f * light_sample.radiance * max(dot(N, light_sample.direction), 0));
                float source_pdf = light_sample.sample_weight;
                float w = target_pdf * source_pdf;

                w_sum += w;
                if(rand(payload.seed) < w / w_sum) {
                    y = u;
                }
                u = rand4(payload.seed);
            }

            sampleAreaLights(light_sample, position, y);
            vec3 wi = toLocal(frame, light_sample.direction);
            vec3 f = evalPrincipledBrdf(alpha, metallic, albedo, wo, wi);

            float target_pdf_y = luminance(f * light_sample.radiance * max(dot(N, light_sample.direction), 0));
            light_sample.sample_weight = (1.0 / target_pdf_y) * (w_sum / M);

            light_sample.sample_weight *= 1.0f / (1.0 - SUN_P);

        } else {
            sampleAreaLights(light_sample, position, u);
            light_sample.sample_weight *= 1.0f / (1.0 - SUN_P);
        }
    }

    RayDesc shadow_ray;
    shadow_ray.Origin = position;
    shadow_ray.Direction = light_sample.direction;
    shadow_ray.TMin = 1.0e-3;
    shadow_ray.TMax = light_sample.shadow_distance;

    HitInfo shadow_payload = {
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        vec3(0, 0, 0),
        0.0,
        uvec4(0, 0, 0, 0),
        0,
        0.0,
    };

    // Shadowing
    TraceRay(scene,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH |
        RAY_FLAG_SKIP_CLOSEST_HIT_SHADER, ~0, 0, 1, 0,
        shadow_ray, shadow_payload);

    // Shading
    if(shadow_payload.distance < 0.0) {
        vec3 wi = toLocal(frame, light_sample.direction);
        vec3 f = evalPrincipledBrdf(alpha, metallic, albedo, wo, wi);
        float w = light_sample.sample_weight;
        if (!light_sample.delta_light && g_constants.sampling_mode == MIS) {
            float brdf_pdf = pdfPrincipledBrdf(alpha, metallic, wi, wo);
            float light_pdf = 1.0f / light_sample.sample_weight;
            w = balance_heuristic(brdf_pdf, light_pdf);
        }
        payload.color += payload.throughput * f * light_sample.radiance * max(dot(N, light_sample.direction), 0) * w;
    }

    // Brdf sampling
    u.xy = rand2(payload.seed);
    vec3 sampled_dir;
    float pdf;
    vec3 f = samplePrincipledBrdf(alpha, metallic, albedo, wo, u.xy, sampled_dir, pdf);

    payload.direction = toWorld(frame, sampled_dir);
    payload.throughput *= pdf > 0.0 ? f / pdf : 0.0;
    payload.brdf_pdf = pdf;
    payload.distance = distance;

    // Debug
    switch(g_constants.debug) {
        case 1: payload.color = roughness; break;
        case 2: payload.color = metallic; break;
        case 3: payload.color = emissive; break;
        case 4: payload.color = N * 0.5 + 0.5; break;
        // case 5: payload.color = light_sample; break;
    }
}
