#ifndef CONTROLLER_CANVAS_H
#define CONTROLLER_CANVAS_H

#include <stdint.h>
#include "input.h"
#include "controller.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ControllerCanvas ControllerCanvas;

ControllerCanvas *controller_canvas_new_v1(void);
/* Events are UTF-8 JSON and are consumed synchronously. The sink is borrowed
 * for the duration of this call and must not re-enter the canvas handle.
 * Configure accepts optional fullViewport (default false): fit and 1x centering
 * use the whole viewport, while insets and padding only restrict remote input.
 * State image coordinates are absolute viewport coordinates; fullViewport
 * callers render against the viewport rather than clipping to the safe rect. */
int32_t controller_canvas_event_v1(ControllerCanvas *canvas, const char *event_json,
    ControllerInputSink sink, void *context);
/* Returned state is owned and must be freed with controller_free_string. */
char *controller_canvas_state_v1(ControllerCanvas *canvas);
/* Clear local pressed/touch state and disable input after the caller has
 * already revoked authorization or released remote input. Keeps view and mode.
 * Use a "release" event with a sink when remote keys/buttons must be released. */
void controller_canvas_suspend_v1(ControllerCanvas *canvas);
void controller_canvas_free_v1(ControllerCanvas *canvas);

#ifdef __cplusplus
}
#endif
#endif
