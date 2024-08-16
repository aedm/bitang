const float GAMMA = 1;

vec3 gamma_compress(vec3 color) {
    return pow(color, vec3(1.0 / GAMMA));
}

vec3 gamma_decompress(vec3 color) {
    return pow(color, vec3(GAMMA));
}