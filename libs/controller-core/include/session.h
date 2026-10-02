#ifndef CONTROLLER_SESSION_H
#define CONTROLLER_SESSION_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ControllerSession ControllerSession;
typedef void (*ControllerSessionCallback)(const char *event_json, void *user);
/* Encoded VP8 bytes are borrowed only until the callback returns. */
typedef void (*ControllerVideoCallback)(const uint8_t *data, uint32_t length, uint32_t width,
    uint32_t height, int64_t pts, uint8_t key_frame, void *user);

/* request_json: literal endpoint, peerId, trusted Base64 peerPublicKey,
 * optional peerFingerprint and minimumKxVersion (only 1 supported).
 * password is UTF-8 and used only for this task. Invalid input returns NULL.
 * Run exactly once on a worker thread. The callback JSON is borrowed until
 * return. Cancel may run on another thread. Destroy only after run returns.
 */
ControllerSession *controller_session_create(const char *request_json, const char *password, uint32_t timeout_ms);
/* Persistent connection; handshake_ms must be 100..60000.
 * request_json expectedPeer defaults to "demo". "secure_host" explicitly
 * selects the non-media official entry. "secure_video" requests read-only VP8
 * and requires controller_session_run_video with a non-NULL video callback.
 * The peer must match the selected mode; there is no automatic fallback.
 * The event includes confirmationCode while awaiting approval, and x/y/textLength
 * for demo_status. Input is accepted only after explicit Keyboard permission.
 */
ControllerSession *controller_connection_create(const char *request_json, const char *password, uint32_t handshake_ms);
/* 0 queued, 1 disconnected, 2 input not authorized, 3 invalid argument, 4 queue full. */
int32_t controller_session_send_pointer(ControllerSession *task, uint32_t x, uint32_t y);
int32_t controller_session_send_text(ControllerSession *task, const char *utf8);
void controller_session_run(ControllerSession *task, ControllerSessionCallback callback, void *user);
void controller_session_run_video(ControllerSession *task, ControllerSessionCallback callback,
    ControllerVideoCallback video_callback, void *user);
void controller_session_cancel(ControllerSession *task);
void controller_session_destroy(ControllerSession *task);

#ifdef __cplusplus
}
#endif
#endif
