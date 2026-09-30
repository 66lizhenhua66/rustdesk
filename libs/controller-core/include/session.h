#ifndef CONTROLLER_SESSION_H
#define CONTROLLER_SESSION_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ControllerSession ControllerSession;
typedef void (*ControllerSessionCallback)(const char *event_json, void *user);

/* request_json: literal endpoint, peerId, trusted Base64 peerPublicKey,
 * optional peerFingerprint and minimumKxVersion (only 1 supported).
 * password is UTF-8 and used only for this task. Invalid input returns NULL.
 * Run exactly once on a worker thread. The callback JSON is borrowed until
 * return. Cancel may run on another thread. Destroy only after run returns.
 */
ControllerSession *controller_session_create(const char *request_json, const char *password, uint32_t timeout_ms);
void controller_session_run(ControllerSession *task, ControllerSessionCallback callback, void *user);
void controller_session_cancel(ControllerSession *task);
void controller_session_destroy(ControllerSession *task);

#ifdef __cplusplus
}
#endif
#endif
