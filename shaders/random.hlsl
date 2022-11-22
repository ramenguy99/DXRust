//RNG from code by Moroz Mykhailo (https://www.shadertoy.com/view/wltcRS)

//internal RNG state
void pcg4d(inout uvec4 v)
{
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    v = v ^ (v >> 16u);
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
}

float rand(uvec4 seed)
{
    pcg4d(seed);
    return float(seed.x)/float(0xffffffffu);
}

vec2 rand2(uvec4 seed)
{
    pcg4d(seed);
    return vec2(seed.xy)/float(0xffffffffu);
}

vec3 rand3(uvec4 seed)
{
    pcg4d(seed);
    return vec3(seed.xyz)/float(0xffffffffu);
}

vec4 rand4(uvec4 seed)
{
    pcg4d(seed);
    return vec4(seed)/float(0xffffffffu);
}
