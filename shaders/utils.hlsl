#ifndef __UTILS_HLSL
#define __UTILS_HLSL

uint murmurMix(uint Hash)
{
	Hash ^= Hash >> 16;
	Hash *= 0x85ebca6b;
	Hash ^= Hash >> 13;
	Hash *= 0xc2b2ae35;
	Hash ^= Hash >> 16;
	return Hash;
}

float3 intToColor(uint Index)
{
	uint Hash = murmurMix(Index);

	float3 Color = float3
	(
		(Hash >>  0) & 255,
		(Hash >>  8) & 255,
		(Hash >> 16) & 255
	);

	return Color * (1.0f / 255.0f);
}

struct Frame {
    mat3 world_from_local;
};

vec3 toWorld(Frame f, vec3 local_v) {
    return mul(f.world_from_local, local_v);
}

vec3 toLocal(Frame f, vec3 world_v) {
    mat3 local_from_world = transpose(f.world_from_local);
    return mul(local_from_world, world_v);
}

Frame frameFromNormal(vec3 a) {
    vec3 b, c;
    if (abs(a.x) > abs(a.y)) {
        float invLen = 1.0f / sqrt(a.x * a.x + a.z * a.z);
        c = vec3(a.z * invLen, 0.0f, -a.x * invLen);
    } else {
        float invLen = 1.0f / sqrt(a.y * a.y + a.z * a.z);
        c = vec3(0.0f, a.z * invLen, -a.y * invLen);
    }
    b = cross(c, a);
    Frame frame;
    frame.world_from_local = transpose(mat3(b, c, a));
    return frame;
}

Frame makeFrame(vec3 t, vec3 b, vec3 n) {
    Frame frame;
    frame.world_from_local = transpose(mat3(t, b, n));
    return frame;
}


float safeSqrt(float x) {
    return sqrt(max(x, 0.0));
}

float square(float x) {
    return x * x;
}

float linearToSRGB(float v) {
    v = clamp(v, 0.0, 1.0);
    if(v > 0.0031308)
    {
        return 1.055 * pow(v, 1.0/2.4) - 0.055;
    }
    else
    {
        return v * 12.92;
    }
}

float luminance(vec3 x) {
    return dot(x, vec3(0.212671, 0.715160, 0.072169));
}

float balance_heuristic(float pa, float pb) {
    return 1.0f / (pa + pb);
}

#endif