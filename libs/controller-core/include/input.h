#ifndef CONTROLLER_INPUT_H
#define CONTROLLER_INPUT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ControllerInput ControllerInput;
/* command_json is borrowed only until this synchronous callback returns.
 * The sink must enforce session authorization (normally send_input_v1).
 * Return 0 only when accepted; any other status aborts the current event.
 */
typedef int32_t (*ControllerInputSink)(void *context, const char *command_json);

/* Single-threaded opaque handle; do not call any input API on the same handle
 * from a sink callback. The engine does not connect or authorize a session.
 */
ControllerInput *controller_input_new_v1(void);
/* UTF-8 JSON, at most 8192 bytes including all fields (excluding the final NUL).
 * Coordinates/deltas use local logical units, finite and within +/-1000000;
 * width/height are nonnegative and no greater than 1000000.
 * Events (all fields required, unknown fields rejected):
 * touch: action down/up/move/cancel, points and changedPoints [{id,x,y}],
 *        time (nonnegative milliseconds), width, height. Max 16 points each.
 * mouse: action press/release/move/cancel, button left/right/middle/empty,
 *        x, y, width, height.
 * wheel: action begin/update/empty, x, y, dx, dy, discrete, width, height.
 *        Discrete deltas must be integers -10..10.
 * key: action down/up/empty, physicalCode (opaque uint32), code (up to 64 bytes).
 *      Unknown/empty semantic keys are ignored. At most 128 held physical keys.
 * text: text (no embedded NUL), split into commands of at most 512 UTF-8 bytes.
 * release: no fields; releases held inputs and clears gesture/scroll state.
 * Return 0 for accepted/ignored events, 1 for invalid arguments, or the first
 * nonzero sink result. A sink failure clears local state and stops all further
 * commands; the caller must stop input through its existing session gate.
 * Successful callbacks mean queued, not delivered to the remote machine.
 */
int32_t controller_input_event_v1(ControllerInput *input, const char *event_json,
    ControllerInputSink sink, void *context);
/* Local reset only: no sink call. Use when disabling or discarding a session. */
void controller_input_reset_v1(ControllerInput *input);
void controller_input_free_v1(ControllerInput *input);

#ifdef __cplusplus
}
#endif
#endif
