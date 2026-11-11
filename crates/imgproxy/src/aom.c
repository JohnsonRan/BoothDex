// AVIF through the system's libavif and its libaom encoder (the `libavif` feature). The few calls live in C so
// that libavif's structs are laid out by the installed header, whatever its version (1.0 and later).

#include <avif/avif.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#ifdef __GLIBC__
#include <malloc.h>
#endif

// libaom starts its worker threads anew for every image, and glibc gives each new thread its own malloc arena
// (up to 8 per core) whose freed memory it keeps: 50 MB more held after every original, and a burst of 24
// originals was killed at a 1 GB limit. With two arenas the same burst peaks at about 260 MB. To be called once,
// before other threads exist; MALLOC_ARENA_MAX in the environment has the same effect and is left alone.
void boothdex_limit_malloc_arenas(void) {
#ifdef __GLIBC__
    mallopt(M_ARENA_MAX, 2);
#endif
}

// One still image, 8-bit, full-resolution chroma (4:4:4), full range, sRGB: what avifenc writes for a PNG.
// `iq`: libaom's image-quality tuning (3.12 and later), 3 to 7% smaller at the same score than its default.
static avifResult encode(const uint8_t *pixels, uint32_t width, uint32_t height, int has_alpha, int quality,
                         int speed, int threads, int iq, avifRWData *out) {
    avifImage *image = avifImageCreate(width, height, 8, AVIF_PIXEL_FORMAT_YUV444);
    if (!image) return AVIF_RESULT_OUT_OF_MEMORY;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
    image->yuvRange = AVIF_RANGE_FULL;

    avifRGBImage rgb;
    avifRGBImageSetDefaults(&rgb, image);
    rgb.format = has_alpha ? AVIF_RGB_FORMAT_RGBA : AVIF_RGB_FORMAT_RGB;
    rgb.depth = 8;
    rgb.pixels = (uint8_t *)pixels; // read only
    rgb.rowBytes = width * (has_alpha ? 4 : 3);

    avifEncoder *encoder = NULL;
    avifResult result = avifImageRGBToYUV(image, &rgb);
    if (result != AVIF_RESULT_OK) goto done;
    encoder = avifEncoderCreate();
    if (!encoder) {
        result = AVIF_RESULT_OUT_OF_MEMORY;
        goto done;
    }
    encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
    encoder->quality = quality;
    encoder->qualityAlpha = quality;
    encoder->speed = speed;
    encoder->maxThreads = threads;
    if (iq) {
        result = avifEncoderSetCodecSpecificOption(encoder, "tune", "iq");
        if (result != AVIF_RESULT_OK) goto done;
    }
    result = avifEncoderWrite(encoder, image, out);
done:
    if (encoder) avifEncoderDestroy(encoder);
    avifImageDestroy(image);
    return result;
}

// 0 and the file in `*out` (released with boothdex_avif_free), or libavif's error code. `pixels` is RGB or RGBA,
// 8 bits a channel, rows packed.
int boothdex_avif_encode(const uint8_t *pixels, uint32_t width, uint32_t height, int has_alpha, int quality,
                         int speed, int threads, uint8_t **out, size_t *out_len) {
    // a libaom from before 3.12 refuses the tuning; its default then, for this and every later image
    static atomic_int iq = 1;
    int tuned = atomic_load(&iq);
    avifRWData data = AVIF_DATA_EMPTY;
    avifResult result = encode(pixels, width, height, has_alpha, quality, speed, threads, tuned, &data);
    if (tuned && result == AVIF_RESULT_INVALID_CODEC_SPECIFIC_OPTION) {
        // said once; images encoded at the same moment may each get here
        if (atomic_exchange(&iq, 0)) fprintf(stderr, "libaom has no tune=iq (older than 3.12): encoding with its default tuning, files a few percent larger\n");
        avifRWDataFree(&data);
        result = encode(pixels, width, height, has_alpha, quality, speed, threads, 0, &data);
    }
    if (result != AVIF_RESULT_OK) {
        avifRWDataFree(&data);
        return (int)result;
    }
    *out = data.data;
    *out_len = data.size;
    return 0;
}

void boothdex_avif_free(uint8_t *data) {
    avifFree(data);
}

// libavif's message for a code boothdex_avif_encode returned
const char *boothdex_avif_error(int code) {
    return avifResultToString((avifResult)code);
}
