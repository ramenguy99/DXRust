//RNG from code by Moroz Mykhailo (https://www.shadertoy.com/view/wltcRS)

//internal RNG state
void pcg4d(inout uvec4 v)
{
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    v = v ^ (v >> 16u);
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
}

float rand(inout uvec4 seed)
{
    pcg4d(seed);
    return float(seed.x)/float(0xffffffffu);
}

vec2 rand2(inout uvec4 seed)
{
    pcg4d(seed);
    return vec2(seed.xy)/float(0xffffffffu);
}

vec3 rand3(inout uvec4 seed)
{
    pcg4d(seed);
    return vec3(seed.xyz)/float(0xffffffffu);
}

vec4 rand4(inout uvec4 seed)
{
    pcg4d(seed);
    return vec4(seed)/float(0xffffffffu);
}

vec3 sampleUniformSphere(inout uvec4 seed) {
    vec2 u = rand2(seed);

    float z = 2.0 * u.x - 1.0;
    float r = sqrt(1.0 - z * z);
    float phi = (2.0 * PI) * u.y;
    float x = r * cos(phi);
    float y = r * sin(phi);
    return vec3(x, y, z);
}

vec3 pdfUniformSphere() {
    return 1.0 / (4.0 * PI);
}


vec2 sampleUniformDisk(inout uvec4 seed) {
    vec2 u = rand2(seed);

    float r = sqrt(u.x);
    float theta = (2.0f * PI) * u.y;
    return vec2(r * cos(theta), r * sin(theta));
}

vec3 sampleUniformHemisphere(inout uvec4 seed) {
    vec3 d = sampleUniformSphere(seed);
    if(d.z < 0.0) {
        d.z = -d.z;
    }
    return d;
}

vec3 sampleCosineWeightedHemisphere(inout uvec4 seed) {
    vec2 p = sampleUniformDisk(seed);
    float z = sqrt(1 - p.x * p.x - p.y * p.y);
    return vec3(p.x, p.y, z);
}

float pdfCosineWeightedHemisphere(vec3 v) {
    return v.z * (1.0 / PI);
}

vec3 pdfUniformHemisphere() {
    return 1.0 / (2.0 * PI);
}


vec3 sampleUniformHemisphereN(inout uvec4 seed, vec3 n) {
    vec3 d = sampleUniformSphere(seed);
    if(dot(d, n) < 0.0f) {
        d = -d;
    }
    return d;
}