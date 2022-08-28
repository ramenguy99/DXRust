#include "defines.hlsl"

GlobalRootSignature MyGlobalRootSignature =
{
    "DescriptorTable(UAV(u0), SRV(t0), SRV(t1), SRV(t2))," // output, as, indices, normals
    "CBV(b0)," // constants
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

struct Constants {
    vec3 camera_position;

    vec3 light_position;

    vec3 diffuse_color;
    float film_dist;

    mat4 projection;
    mat4 view;
    mat4 model;
    mat4 normal;
};

ConstantBuffer<Constants> g_constants: register(b0);

struct HitInfo
{
    vec3 color;
    float distance;
};

inline void GenerateCameraRay(uint2 index, out float3 origin, out float3 direction)
{
    float2 xy = index + 0.5f; // center in the middle of the pixel.
    float2 offset = xy / DispatchRaysDimensions().xy * 2.0 - 1.0;

    vec3 camera_forward = vec3(0, 1, 0);
    vec3 camera_up = vec3(0, 0, 1);
    vec3 camera_right = vec3(1, 0, 0);
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
    vec3 dir;
    vec3 origin;
    GenerateCameraRay(DispatchRaysIndex().xy, origin, dir);

    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = dir;
    ray.TMin = 0.01;
    ray.TMax = 1000.0;

    HitInfo payload = { vec3(0, 0, 0), 0.0 };
    TraceRay(scene, RAY_FLAG_CULL_BACK_FACING_TRIANGLES, ~0, 0, 1, 0, ray, payload);
    
    output[DispatchRaysIndex().xy].xyz = payload.color;
    output[DispatchRaysIndex().xy].w = 1.0;
}


[shader("miss")]
void Miss(inout HitInfo payload)
{
    payload.color = vec3(0.1, 0.1, 0.1);
    payload.distance = -1.0;
}

[shader("closesthit")]
void ClosestHit(inout HitInfo payload, in BuiltInTriangleIntersectionAttributes attribs)
{
    vec3 position = WorldRayOrigin() + RayTCurrent() * WorldRayDirection();

    uint triangle_index = PrimitiveIndex();
    
    uint3 indices;
    indices.x = index_buffer[triangle_index * 3 + 0];
    indices.y = index_buffer[triangle_index * 3 + 1];
    indices.z = index_buffer[triangle_index * 3 + 2];

    vec2 barycentrics = attribs.barycentrics;

    vec3 normal = 
        normals_buffer[indices.x] * (1 - barycentrics.x - barycentrics.y) +
        normals_buffer[indices.y] * barycentrics.x +
        normals_buffer[indices.z] * barycentrics.y;
    
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
    vec3 ks = pow(max(dot(N, H), 0), 16.0);

    vec3 color = diffuse * (ka + kd) + specular * ks;
    //vec3 color = specular * (PrimitiveIndex() / 100000.0);

    //vec3 color = N * 0.5 + 1.0;
    //vec3 color = vec3(barycentrics, 1- barycentrics.x - barycentrics.y);

    payload.color = color;
    payload.distance = RayTCurrent();
}
