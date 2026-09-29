#include <napi/native_api.h>

#include <cstdint>
#include <new>
#include <thread>

#include "probe.h"

namespace {

struct Event {
    uint32_t task_id;
    uint32_t kind;
    uint32_t step;
    uint32_t total;
};

struct Task {
    uint32_t id;
    ProbeTask *probe;
    napi_threadsafe_function tsfn;
    std::thread worker;
};

struct State {
    Task *task = nullptr;
    uint32_t next_id = 1;
};

void Throw(napi_env env, const char *code, const char *message) {
    napi_value text = nullptr;
    napi_value error = nullptr;
    napi_value code_value = nullptr;
    if (napi_create_string_utf8(env, message, NAPI_AUTO_LENGTH, &text) != napi_ok ||
        napi_create_error(env, nullptr, text, &error) != napi_ok ||
        napi_create_string_utf8(env, code, NAPI_AUTO_LENGTH, &code_value) != napi_ok ||
        napi_set_named_property(env, error, "code", code_value) != napi_ok) {
        napi_throw_error(env, code, message);
        return;
    }
    napi_throw(env, error);
}

bool Number(napi_env env, napi_value value, uint32_t min, uint32_t max, uint32_t *out) {
    napi_valuetype type;
    double number;
    if (napi_typeof(env, value, &type) != napi_ok || type != napi_number ||
        napi_get_value_double(env, value, &number) != napi_ok ||
        !(number >= min && number <= max) || number != static_cast<uint32_t>(number)) {
        return false;
    }
    *out = static_cast<uint32_t>(number);
    return true;
}

State *GetState(napi_env env) {
    State *state = nullptr;
    if (napi_get_instance_data(env, reinterpret_cast<void **>(&state)) != napi_ok) {
        return nullptr;
    }
    return state;
}

void CallJs(napi_env env, napi_value callback, void *, void *data) {
    Event *event = static_cast<Event *>(data);
    if (event == nullptr) {
        return;
    }
    if (env != nullptr && callback != nullptr) {
        napi_value value = nullptr;
        napi_value task_id = nullptr;
        napi_value kind = nullptr;
        napi_value step = nullptr;
        napi_value total = nullptr;
        const char *kind_name = event->kind == 1 ? "progress" : event->kind == 2 ? "completed" : "cancelled";
        if (napi_create_object(env, &value) == napi_ok &&
            napi_create_uint32(env, event->task_id, &task_id) == napi_ok &&
            napi_create_string_utf8(env, kind_name, NAPI_AUTO_LENGTH, &kind) == napi_ok &&
            napi_create_uint32(env, event->step, &step) == napi_ok &&
            napi_create_uint32(env, event->total, &total) == napi_ok &&
            napi_set_named_property(env, value, "taskId", task_id) == napi_ok &&
            napi_set_named_property(env, value, "kind", kind) == napi_ok &&
            napi_set_named_property(env, value, "step", step) == napi_ok &&
            napi_set_named_property(env, value, "total", total) == napi_ok) {
            napi_value global = nullptr;
            napi_value ignored = nullptr;
            if (napi_get_global(env, &global) == napi_ok) {
                napi_call_function(env, global, callback, 1, &value, &ignored);
            }
        }
    }
    delete event;
}

void OnProbeEvent(uint32_t kind, uint32_t step, uint32_t total, void *user) {
    Task *task = static_cast<Task *>(user);
    Event *event = new (std::nothrow) Event{task->id, kind, step, total};
    if (event != nullptr && napi_call_threadsafe_function(task->tsfn, event, napi_tsfn_nonblocking) != napi_ok) {
        delete event;
    }
}

void Stop(State *state) {
    Task *task = state->task;
    if (task == nullptr) {
        return;
    }
    state->task = nullptr;
    probe_cancel(task->probe);
    if (task->worker.joinable()) {
        task->worker.join();
    }
    napi_release_threadsafe_function(task->tsfn, napi_tsfn_abort);
    probe_destroy(task->probe);
    delete task;
}

void Cleanup(void *data) {
    State *state = static_cast<State *>(data);
    Stop(state);
    delete state;
}

napi_value Version(napi_env env, napi_callback_info) {
    napi_value value = nullptr;
    if (napi_create_string_utf8(env, probe_version(), NAPI_AUTO_LENGTH, &value) != napi_ok) {
        Throw(env, "NATIVE_FAILURE", "Unable to create version string");
        return nullptr;
    }
    return value;
}

napi_value Start(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value args[3] = {};
    uint32_t steps = 0;
    uint32_t interval = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 3 ||
        !Number(env, args[0], 1, 100, &steps) || !Number(env, args[1], 10, 1000, &interval) ||
        napi_typeof(env, args[2], &callback_type) != napi_ok || callback_type != napi_function) {
        Throw(env, "INVALID_ARGUMENT", "Expected steps (1..100), interval (10..1000 ms), callback");
        return nullptr;
    }
    State *state = GetState(env);
    if (state == nullptr) {
        Throw(env, "NATIVE_FAILURE", "Probe state is unavailable");
        return nullptr;
    }
    Stop(state);
    ProbeTask *probe = probe_create(steps, interval);
    if (probe == nullptr) {
        Throw(env, "INVALID_ARGUMENT", "Invalid probe parameters");
        return nullptr;
    }
    napi_value resource_name = nullptr;
    napi_threadsafe_function tsfn = nullptr;
    if (napi_create_string_utf8(env, "HarmonyProbe", NAPI_AUTO_LENGTH, &resource_name) != napi_ok ||
        napi_create_threadsafe_function(env, args[2], nullptr, resource_name, 0, 2, nullptr,
                                        nullptr, nullptr, CallJs, &tsfn) != napi_ok) {
        probe_destroy(probe);
        Throw(env, "NATIVE_FAILURE", "Unable to create callback bridge");
        return nullptr;
    }
    uint32_t id = state->next_id++;
    if (id == 0) {
        id = state->next_id++;
    }
    Task *task = new (std::nothrow) Task{id, probe, tsfn, {}};
    if (task == nullptr) {
        napi_release_threadsafe_function(tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(tsfn, napi_tsfn_abort);
        probe_destroy(probe);
        Throw(env, "NATIVE_FAILURE", "Unable to allocate task");
        return nullptr;
    }
    try {
        task->worker = std::thread([task]() {
            probe_run(task->probe, OnProbeEvent, task);
            napi_release_threadsafe_function(task->tsfn, napi_tsfn_release);
        });
    } catch (...) {
        napi_release_threadsafe_function(tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(tsfn, napi_tsfn_abort);
        probe_destroy(probe);
        delete task;
        Throw(env, "NATIVE_FAILURE", "Unable to start worker");
        return nullptr;
    }
    state->task = task;
    napi_value result = nullptr;
    if (napi_create_uint32(env, id, &result) != napi_ok) {
        Stop(state);
        Throw(env, "NATIVE_FAILURE", "Unable to return task ID");
        return nullptr;
    }
    return result;
}

napi_value Cancel(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1] = {};
    uint32_t id = 0;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 1 ||
        !Number(env, args[0], 1, UINT32_MAX, &id)) {
        Throw(env, "INVALID_ARGUMENT", "Expected a task ID");
        return nullptr;
    }
    State *state = GetState(env);
    if (state == nullptr) {
        Throw(env, "NATIVE_FAILURE", "Probe state is unavailable");
        return nullptr;
    }
    bool found = state->task != nullptr && state->task->id == id;
    if (found) {
        Stop(state);
    }
    napi_value result = nullptr;
    if (napi_get_boolean(env, found, &result) != napi_ok) {
        Throw(env, "NATIVE_FAILURE", "Unable to return cancellation result");
        return nullptr;
    }
    return result;
}

napi_value Dispose(napi_env env, napi_callback_info) {
    State *state = GetState(env);
    if (state == nullptr) {
        Throw(env, "NATIVE_FAILURE", "Probe state is unavailable");
        return nullptr;
    }
    Stop(state);
    napi_value result = nullptr;
    if (napi_get_undefined(env, &result) != napi_ok) {
        Throw(env, "NATIVE_FAILURE", "Unable to return from dispose");
        return nullptr;
    }
    return result;
}

napi_value Init(napi_env env, napi_value exports) {
    State *state = new (std::nothrow) State;
    if (state == nullptr) {
        Throw(env, "NATIVE_FAILURE", "Unable to allocate probe state");
        return nullptr;
    }
    if (napi_set_instance_data(env, state, nullptr, nullptr) != napi_ok) {
        delete state;
        Throw(env, "NATIVE_FAILURE", "Unable to initialize probe state");
        return nullptr;
    }
    if (napi_add_env_cleanup_hook(env, Cleanup, state) != napi_ok) {
        napi_set_instance_data(env, nullptr, nullptr, nullptr);
        delete state;
        Throw(env, "NATIVE_FAILURE", "Unable to register cleanup");
        return nullptr;
    }
    napi_property_descriptor methods[] = {
        {"version", nullptr, Version, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"start", nullptr, Start, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"cancel", nullptr, Cancel, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"dispose", nullptr, Dispose, nullptr, nullptr, nullptr, napi_default, nullptr},
    };
    if (napi_define_properties(env, exports, sizeof(methods) / sizeof(methods[0]), methods) != napi_ok) {
        napi_remove_env_cleanup_hook(env, Cleanup, state);
        napi_set_instance_data(env, nullptr, nullptr, nullptr);
        delete state;
        Throw(env, "NATIVE_FAILURE", "Unable to export probe methods");
        return nullptr;
    }
    return exports;
}

napi_module module = {1, 0, nullptr, Init, "probe", nullptr, {0}};

} // namespace

extern "C" __attribute__((constructor)) void RegisterProbeModule() {
    napi_module_register(&module);
}
