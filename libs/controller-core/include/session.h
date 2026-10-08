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
 * "secure_control" requests versioned VP8 and separately authorized input;
 * it also requires controller_session_run_video.
 * Its optional videoQuality (low/balanced/high) and videoFps (10/15/30) must
 * appear together and require the peer's video settings extension. Confirmed
 * settings arrive as video_settings events before video at the new dimensions.
 * The peer must match the selected mode; there is no automatic fallback.
 * The event includes confirmationCode while awaiting approval, and x/y/textLength
 * for demo_status. Demo input requires Keyboard permission; secure_control input
 * requires a matching v2 controller request and host state with a current token.
 */
ControllerSession *controller_connection_create(const char *request_json, const char *password, uint32_t handshake_ms);
/* Secret JSON must be kept outside profiles and destroyed with the wiping free function.
 * Access requires expectedPeer=secure_video and controller_session_run_video.
 */
char *controller_identity_create_v1(void);
void controller_secret_string_free_v1(char *value);
ControllerSession *controller_connection_create_access_v1(const char *request_json,
    const char *credential_json, uint32_t timeout_ms);
/* 0 queued, 1 disconnected, 2 input not authorized, 3 invalid argument, 4 queue full. */
int32_t controller_session_send_pointer(ControllerSession *task, uint32_t x, uint32_t y);
int32_t controller_session_send_text(ControllerSession *task, const char *utf8);
/* JSON command is one of move/button/wheel/key/text/release_all. Token stays inside Rust.
 * Returns the same 0..4 status codes as the demo send functions.
 */
int32_t controller_session_send_input_v1(ControllerSession *task, const char *command_json);
/* 0 queued, 1 disconnected or wrong mode, 2 unavailable, 3 invalid boolean, 4 queue full.
 * Closing input invalidates local authorization and pending input immediately.
 */
int32_t controller_session_set_input_enabled_v1(ControllerSession *task, uint8_t enabled);
/* JSON {"quality":"low|balanced|high","fps":10|15|30}; connected secure_control
 * sessions with negotiated video settings only. Input authorization is not required.
 * 0 queued, 1 inactive/wrong mode, 2 unsupported, 3 invalid JSON, 4 request pending.
 * The video_settings event confirms the applied choice; missing confirmation fails
 * the session after 5 seconds. No new login or input grant is requested.
 */
int32_t controller_session_set_video_settings_v1(ControllerSession *task, const char *settings_json);
void controller_session_run(ControllerSession *task, ControllerSessionCallback callback, void *user);
void controller_session_run_video(ControllerSession *task, ControllerSessionCallback callback,
    ControllerVideoCallback video_callback, void *user);
void controller_session_cancel(ControllerSession *task);
void controller_session_destroy(ControllerSession *task);

#ifdef __cplusplus
}
#endif
#endif
