
vec3 fresnelSchlick(float cos_theta, vec3 F0) {
    return F0 + (1 - F0) * pow(1 - cos_theta, 5);
}

float tanTheta2(vec3 v) {
    float sin2 = 1 - v.z * v.z;
    if (sin2 <= 0.0)
        return 0.0;
    return sin2 / (v.z * v.z);
}

float evalGeometryGGX(vec3 v, float alpha) {
    return 2.0 / (1.0 + sqrt(1.0 + square(alpha) * tanTheta2(v)));
}

vec3 evalPrincipledBrdfHelper(float alpha, float metallic, vec3 base_color,
                              vec3 wo, vec3 wh, vec3 wi) {
    float cos_theta_h = wh.z;
    float cos_theta_d = dot(wh, wo);

    vec3 diffuse = (1.0f - metallic) * base_color * INV_PI;

    float D = evalGTR2(cos_theta_h, alpha);

    vec3 specular_color = lerp(0.04, base_color, metallic);
    vec3 F = fresnelSchlick(cos_theta_d, specular_color);

    float G = evalGeometryGGX(wi, alpha) * evalGeometryGGX(wo, alpha);
    vec3 specular = (D * F * G) / (4.0 * wi.z * wo.z);

    return diffuse + specular;
}

vec3 evalPrincipledBrdf(float alpha, float metallic, vec3 base_color,
                        vec3 wo, vec3 wi) {
    if (wi.z <= 0.0 || wo.z <= 0.0) {
        return 0.0;
    }

    vec3 wh = normalize(wo + wi);
    return evalPrincipledBrdfHelper(alpha, metallic, base_color, wo, wh, wi);
}

float pdfPrincipledBrdfHelper(float alpha, float metallic, vec3 wo, vec3 wh) {
    float s_pdf = pdfGTR2(wh, alpha) / (4 * dot(wh, wo));
    float d_pdf = pdfCosineWeightedHemisphere(wo);
    return d_pdf * (1.0 - metallic) +  metallic * s_pdf;
}

float pdfPrincipledBrdf(float alpha, float metallic, vec3 wo, vec3 wi) {
    if (wo.z <= 0.0 || wi.z <= 0.0) {
        return 0.0;
    }
    vec3 wh = normalize(wo + wi);
    return pdfPrincipledBrdfHelper(alpha, metallic, wo, wh);
}

vec3 samplePrincipledBrdf(float alpha, float metallic, vec3 base_color,
                          vec3 wi, vec3 u, out vec3 wo, out float pdf) {
    float diffuse_weight = 1.0 - metallic;
    vec3 wh;
    if (u.x < diffuse_weight) {
        wo = sampleCosineWeightedHemisphere(u.yz);
        wh = normalize(wo + wi);
    } else {
        wh = sampleGTR2(u.yz, alpha);
        wo = 2 * dot(wi, wh) * wh - wi;
    }

    pdf = pdfPrincipledBrdfHelper(alpha, metallic, wo, wh);
    if(pdf > 0.0) {
        return evalPrincipledBrdfHelper(alpha, metallic, base_color, wo, wh, wi) * wo.z;
    } else {
        return 0.0;
    }
}