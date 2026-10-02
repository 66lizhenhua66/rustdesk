#pragma once

#include <cstddef>
#include <cstdint>
#include <mutex>
#include <string>

#include <native_window/external_window.h>
#include <vpx/vpx_decoder.h>

class VideoRenderer {
public:
    explicit VideoRenderer(const std::string& surfaceId);
    ~VideoRenderer();

    VideoRenderer(const VideoRenderer&) = delete;
    VideoRenderer& operator=(const VideoRenderer&) = delete;

    bool Ready() const;
    bool Submit(const uint8_t* data, size_t size, uint32_t width, uint32_t height,
                int64_t pts, bool key);
    uint64_t PresentedFrames() const;
    void Stop();

private:
    bool Present(const vpx_image_t* image, uint32_t width, uint32_t height);

    mutable std::mutex mutex_;
    OHNativeWindow* window_ = nullptr;
    vpx_codec_ctx_t codec_{};
    bool decoderReady_ = false;
    bool stopped_ = false;
    bool sawKey_ = false;
    uint32_t width_ = 0;
    uint32_t height_ = 0;
    uint64_t presentedFrames_ = 0;
};
