#ifndef __TYPES_HLSL
#define __TYPES_HLSL

#include "defines.hlsl"

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

    float lights_pdf_normalization;
    u32 num_lights;
    u32 bounces;
    u32 sampling_mode;

    u32 use_alias_table;
};

struct RayMeshInstance {
    u32 vertex_offset;
    u32 index_offset;

    u32 albedo_index;
    u32 normal_index;
    u32 specular_index;
    u32 emissive_index;

    vec4 albedo_value;
    vec4 specular_value;
    vec4 emissive_value;
};

struct RasterMeshInstance {
    mat4 transform;

    u32 albedo_index;
    u32 normal_index;
    u32 specular_index;
    u32 emissive_index;

    vec4 albedo_value;
    vec4 specular_value;
    vec4 emissive_value;
};

struct Light {
    vec3 p0;
    vec3 p1;
    vec3 p2;
    vec3 emissive;
};

struct Alias {
    float p;
    u32 a;
};


#endif