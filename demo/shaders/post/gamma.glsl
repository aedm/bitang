const float GAMMA = 2.2;
const float COLOR_BASE_LEVEL = 1;

vec3 gamma_compress(vec3 color) {
    return pow(color, vec3(1.0 / GAMMA));
}

vec3 gamma_decompress(vec3 color) {
    return pow(color, vec3(GAMMA));
}