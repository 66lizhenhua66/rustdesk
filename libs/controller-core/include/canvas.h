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
 * for the duration of this call and must not re-enter the canvas handle. */
int32_t controller_canvas_event_v1(ControllerCanvas *canvas, const char *event_json,
    ControllerInputSink sink, void *context);
/* Returned state is owned and must be freed with controller_free_string. */
char *controller_canvas_state_v1(ControllerCanvas *canvas);
void controller_canvas_free_v1(ControllerCanvas *canvas);

#ifdef __cplusplus
}
#endif
#endif
