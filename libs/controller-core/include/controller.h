#ifndef CONTROLLER_CORE_H
#define CONTROLLER_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ControllerProbe ControllerProbe;
typedef void (*ControllerProbeCallback)(const char *event_json, void *user);

/* version is borrowed static UTF-8; do not free it. */
const char *controller_version(void);
/* validate_profile returns owned UTF-8; release with controller_free_string. */
char *controller_validate_profile(const char *profile_json);
void controller_free_string(char *value);

/* endpoint must be a literal IPv4/IPv6 socket address, including port.
 * timeout_ms must be 100..2000. NULL means invalid arguments.
 * run may be called exactly once, on a caller-owned worker thread. Callback
 * event_json is borrowed until callback return; copy it before returning.
 * cancel is nonblocking and may be called from another thread. Call destroy
 * only after run returns. No function sends application data or credentials.
 */
ControllerProbe *controller_probe_create(const char *endpoint, uint32_t timeout_ms);
void controller_probe_run(ControllerProbe *task, ControllerProbeCallback callback, void *user);
void controller_probe_cancel(ControllerProbe *task);
void controller_probe_destroy(ControllerProbe *task);

#ifdef __cplusplus
}
#endif
#endif
