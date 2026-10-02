#include "video_renderer.h"

#include <algorithm>
#include <charconv>
#include <climits>
#include <cstring>
#include <poll.h>
#include <unistd.h>

#include <native_buffer/native_buffer.h>
#include <vpx/vp8dx.h>

namespace {
constexpr size_t kMaxFrameBytes = 2 * 1024 * 1024;
constexpr uint32_t kMaxWidth = 1280;
constexpr uint32_t kMaxHeight = 720;

bool ValidDimensions(uint32_t width, uint32_t height) {
    return width >= 2 && height >= 2 && width <= kMaxWidth && height <= kMaxHeight &&
           (width % 2) == 0 && (height % 2) == 0;
}

bool ValidKeyHeader(const uint8_t* data, size_t size, uint32_t width, uint32_t height) {
    if (size < 10 || (data[0] & 1) != 0 || (data[0] & 0x0e) != 0 ||
        data[3] != 0x9d || data[4] != 0x01 || data[5] != 0x2a) {
        return false;
    }
    const uint32_t encodedWidth = (data[6] | (uint32_t(data[7]) << 8)) & 0x3fff;
    const uint32_t encodedHeight = (data[8] | (uint32_t(data[9]) << 8)) & 0x3fff;
    vpx_codec_stream_info_t info{};
    info.sz = sizeof(info);
    return encodedWidth == width && encodedHeight == height &&
        vpx_codec_peek_stream_info(vpx_codec_vp8_dx(), data,
            static_cast<unsigned int>(size), &info) == VPX_CODEC_OK &&
        info.is_kf != 0 && info.w == width && info.h == height;
}

uint8_t Clamp(int value) {
    return static_cast<uint8_t>(value < 0 ? 0 : value > 255 ? 255 : value);
}

bool WaitFence(int fd) {
    if (fd < 0) return true;
    pollfd descriptor{fd, POLLIN, 0};
    const int result = poll(&descriptor, 1, 20);
    close(fd);
    return result > 0 && (descriptor.revents & POLLIN) != 0 &&
        (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL)) == 0;
}

void ClearSurface(OHNativeWindow* window, uint32_t width, uint32_t height) {
    OHNativeWindowBuffer* windowBuffer = nullptr;
    int fence = -1;
    if (OH_NativeWindow_NativeWindowRequestBuffer(window, &windowBuffer, &fence) != 0 || !windowBuffer) {
        if (fence >= 0) close(fence);
        return;
    }
    if (!WaitFence(fence)) {
        OH_NativeWindow_NativeWindowAbortBuffer(window, windowBuffer);
        return;
    }
    OH_NativeBuffer* nativeBuffer = nullptr;
    OH_NativeBuffer_Config config{};
    if (OH_NativeBuffer_FromNativeWindowBuffer(windowBuffer, &nativeBuffer) != 0 || !nativeBuffer) {
        OH_NativeWindow_NativeWindowAbortBuffer(window, windowBuffer);
        return;
    }
    OH_NativeBuffer_GetConfig(nativeBuffer, &config);
    BufferHandle* handle = OH_NativeWindow_GetBufferHandleFromNative(windowBuffer);
    if (config.width != static_cast<int32_t>(width) || config.height != static_cast<int32_t>(height) ||
        config.stride < static_cast<int32_t>(width * 4) || !handle ||
        handle->size < static_cast<int64_t>(config.stride) * height) {
        OH_NativeWindow_NativeWindowAbortBuffer(window, windowBuffer);
        return;
    }
    void* mapped = nullptr;
    if (OH_NativeBuffer_Map(nativeBuffer, &mapped) != 0 || !mapped) {
        OH_NativeWindow_NativeWindowAbortBuffer(window, windowBuffer);
        return;
    }
    std::memset(mapped, 0, static_cast<size_t>(config.stride) * height);
    if (OH_NativeBuffer_Unmap(nativeBuffer) == 0) {
        Region region{nullptr, 0};
        if (OH_NativeWindow_NativeWindowFlushBuffer(window, windowBuffer, -1, region) == 0) return;
    }
    OH_NativeWindow_NativeWindowAbortBuffer(window, windowBuffer);
}
} // namespace

VideoRenderer::VideoRenderer(const std::string& surfaceId) {
    uint64_t id = 0;
    const char* first = surfaceId.data();
    const char* last = first + surfaceId.size();
    const auto parsed = std::from_chars(first, last, id);
    if (first == last || parsed.ec != std::errc{} || parsed.ptr != last || id == 0) {
        return;
    }
    if (OH_NativeWindow_CreateNativeWindowFromSurfaceId(id, &window_) != 0) {
        window_ = nullptr;
        return;
    }
    if (OH_NativeWindow_NativeWindowHandleOpt(window_, SET_FORMAT, NATIVEBUFFER_PIXEL_FMT_RGBA_8888) != 0 ||
        OH_NativeWindow_NativeWindowHandleOpt(window_, SET_USAGE,
            uint64_t(NATIVEBUFFER_USAGE_CPU_READ | NATIVEBUFFER_USAGE_CPU_WRITE)) != 0 ||
        OH_NativeWindow_NativeWindowHandleOpt(window_, SET_TIMEOUT, int32_t(0)) != 0) {
        OH_NativeWindow_DestroyNativeWindow(window_);
        window_ = nullptr;
    }
}

VideoRenderer::~VideoRenderer() { Stop(); }

bool VideoRenderer::Ready() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return !stopped_ && window_ != nullptr;
}

uint64_t VideoRenderer::PresentedFrames() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return presentedFrames_;
}

bool VideoRenderer::Submit(const uint8_t* data, size_t size, uint32_t width, uint32_t height,
                           int64_t /*pts*/, bool key) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (stopped_ || !window_ || !data || size == 0 || size > kMaxFrameBytes ||
        !ValidDimensions(width, height) || size > UINT_MAX) return false;
    const bool encodedKey = (data[0] & 1) == 0;
    if (key != encodedKey || (!sawKey_ && !key) ||
        (key && !ValidKeyHeader(data, size, width, height)) ||
        (sawKey_ && (width != width_ || height != height_))) return false;
    if (!decoderReady_) {
        if (vpx_codec_dec_init(&codec_, vpx_codec_vp8_dx(), nullptr, 0) != VPX_CODEC_OK) return false;
        decoderReady_ = true;
    }
    if (vpx_codec_decode(&codec_, data, static_cast<unsigned int>(size), nullptr, 0) != VPX_CODEC_OK) return false;
    if (!sawKey_) {
        sawKey_ = true;
        width_ = width;
        height_ = height;
        if (OH_NativeWindow_NativeWindowHandleOpt(window_, SET_BUFFER_GEOMETRY,
            int32_t(width), int32_t(height)) != 0) return false;
        if (OH_NativeWindow_NativeWindowSetScalingModeV2(
            window_, OH_SCALING_MODE_SCALE_TO_WINDOW_V2) != 0) return false;
    }
    vpx_codec_iter_t iterator = nullptr;
    vpx_image_t* image = vpx_codec_get_frame(&codec_, &iterator);
    if (!image) return true;
    if (image->d_w != width || image->d_h != height || image->fmt != VPX_IMG_FMT_I420 ||
        !image->planes[VPX_PLANE_Y] || !image->planes[VPX_PLANE_U] || !image->planes[VPX_PLANE_V] ||
        image->stride[VPX_PLANE_Y] < static_cast<int>(width) ||
        image->stride[VPX_PLANE_U] < static_cast<int>(width / 2) ||
        image->stride[VPX_PLANE_V] < static_cast<int>(width / 2)) return false;
    return Present(image, width, height);
}

bool VideoRenderer::Present(const vpx_image_t* image, uint32_t width, uint32_t height) {
    if (OH_NativeWindow_NativeWindowHandleOpt(window_, SET_BUFFER_GEOMETRY,
        int32_t(width), int32_t(height)) != 0) return false;
    OHNativeWindowBuffer* windowBuffer = nullptr;
    int fence = -1;
    if (OH_NativeWindow_NativeWindowRequestBuffer(window_, &windowBuffer, &fence) != 0 || !windowBuffer) {
        if (fence >= 0) close(fence);
        return true; // The decoder still needs this frame as a prediction reference.
    }
    if (!WaitFence(fence)) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return true;
    }
    OH_NativeBuffer* nativeBuffer = nullptr;
    if (OH_NativeBuffer_FromNativeWindowBuffer(windowBuffer, &nativeBuffer) != 0 || !nativeBuffer) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return false;
    }
    OH_NativeBuffer_Config config{};
    OH_NativeBuffer_GetConfig(nativeBuffer, &config);
    BufferHandle* handle = OH_NativeWindow_GetBufferHandleFromNative(windowBuffer);
    if (config.width != static_cast<int32_t>(width) || config.height != static_cast<int32_t>(height)) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return true;
    }
    if (config.format != NATIVEBUFFER_PIXEL_FMT_RGBA_8888 ||
        config.stride < static_cast<int32_t>(width * 4) ||
        !handle || handle->size < static_cast<int64_t>(config.stride) * height) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return false;
    }
    void* mapped = nullptr;
    if (OH_NativeBuffer_Map(nativeBuffer, &mapped) != 0 || !mapped) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return false;
    }
    for (uint32_t y = 0; y < height; ++y) {
        auto* row = static_cast<uint8_t*>(mapped) + y * config.stride;
        const uint8_t* luma = image->planes[VPX_PLANE_Y] + y * image->stride[VPX_PLANE_Y];
        const uint8_t* cb = image->planes[VPX_PLANE_U] + (y / 2) * image->stride[VPX_PLANE_U];
        const uint8_t* cr = image->planes[VPX_PLANE_V] + (y / 2) * image->stride[VPX_PLANE_V];
        for (uint32_t x = 0; x < width; ++x) {
            const int c = std::max(0, int(luma[x]) - 16);
            const int d = int(cb[x / 2]) - 128;
            const int e = int(cr[x / 2]) - 128;
            row[4 * x] = Clamp((298 * c + 409 * e + 128) >> 8);
            row[4 * x + 1] = Clamp((298 * c - 100 * d - 208 * e + 128) >> 8);
            row[4 * x + 2] = Clamp((298 * c + 516 * d + 128) >> 8);
            row[4 * x + 3] = 255;
        }
    }
    if (OH_NativeBuffer_Unmap(nativeBuffer) != 0) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return false;
    }
    Region region{nullptr, 0};
    if (OH_NativeWindow_NativeWindowFlushBuffer(window_, windowBuffer, -1, region) != 0) {
        OH_NativeWindow_NativeWindowAbortBuffer(window_, windowBuffer);
        return false;
    }
    ++presentedFrames_;
    return true;
}

void VideoRenderer::Stop() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (stopped_) return;
    stopped_ = true;
    if (decoderReady_) {
        vpx_codec_destroy(&codec_);
        decoderReady_ = false;
    }
    if (window_) {
        if (sawKey_) ClearSurface(window_, width_, height_);
        OH_NativeWindow_DestroyNativeWindow(window_);
        window_ = nullptr;
    }
}
