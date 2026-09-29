#ifndef HARMONY_PROBE_H
#define HARMONY_PROBE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ProbeTask ProbeTask;
typedef void (*ProbeCallback)(uint32_t kind, uint32_t step, uint32_t total, void *user);

const char *probe_version(void);
ProbeTask *probe_create(uint32_t steps, uint32_t interval_ms);
void probe_run(ProbeTask *task, ProbeCallback callback, void *user);
void probe_cancel(ProbeTask *task);
/* Call only after probe_run has returned. */
void probe_destroy(ProbeTask *task);

#ifdef __cplusplus
}
#endif

#endif
