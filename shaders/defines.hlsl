typedef float2 vec2;
typedef float3 vec3;
typedef float4 vec4;

typedef int2 ivec2;
typedef int3 ivec3;
typedef int4 ivec4;

typedef float3x3 mat3;
typedef matrix mat4;

typedef uint u32;

#define PI 3.1415926535897932384626433832795f

float safe_sqrt(float x) {
    return sqrt(max(x, 0.0));
}