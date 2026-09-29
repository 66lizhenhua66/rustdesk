#include <napi/native_api.h>
#include <atomic>
#include <cstdint>
#include <memory>
#include <mutex>
#include <new>
#include <string>
#include <thread>

#include "controller.h"

namespace {
struct Job : std::enable_shared_from_this<Job> {
    uint32_t id = 0;
    ControllerProbe *probe = nullptr;
    napi_threadsafe_function tsfn = nullptr;
    std::atomic<bool> closing{false};
    std::mutex delivery;
    ~Job() { controller_probe_destroy(probe); }
};

struct Event {
    std::shared_ptr<Job> job;
    std::string json;
};

struct State {
    std::shared_ptr<Job> active;
    std::shared_ptr<std::atomic<uint32_t>> workers = std::make_shared<std::atomic<uint32_t>>(0);
    uint32_t next_id = 1;
};

void Error(napi_env env, const char *code, const char *message) {
    napi_throw_error(env, code, message);
}

State *GetState(napi_env env) {
    void *state = nullptr;
    return napi_get_instance_data(env, &state) == napi_ok ? static_cast<State *>(state) : nullptr;
}

bool ReadString(napi_env env, napi_value value, std::string &output, size_t limit) {
    napi_valuetype type;
    size_t length = 0;
    if (napi_typeof(env, value, &type) != napi_ok || type != napi_string ||
        napi_get_value_string_utf8(env, value, nullptr, 0, &length) != napi_ok || length > limit) {
        return false;
    }
    output.resize(length + 1);
    size_t copied = 0;
    if (napi_get_value_string_utf8(env, value, output.data(), output.size(), &copied) != napi_ok) {
        return false;
    }
    output.resize(copied);
    return output.find('\0') == std::string::npos;
}

napi_value String(napi_env env, const char *value) {
    napi_value result = nullptr;
    if (value == nullptr || napi_create_string_utf8(env, value, NAPI_AUTO_LENGTH, &result) != napi_ok) {
        Error(env, "NATIVE_FAILURE", "Unable to return core result");
        return nullptr;
    }
    return result;
}

void Close(State *state) {
    auto job = std::move(state->active);
    if (!job) { return; }
    std::lock_guard<std::mutex> guard(job->delivery);
    job->closing = true;
    controller_probe_cancel(job->probe);
    // The worker retains its own TSFN reference until its bounded socket operation ends.
    napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
}

void Cleanup(void *data) {
    auto *state = static_cast<State *>(data);
    Close(state);
    delete state;
}

void Finalize(napi_env, void *data, void *) {
    delete static_cast<std::shared_ptr<Job> *>(data);
}

void CallJs(napi_env env, napi_value callback, void *, void *data) {
    std::unique_ptr<Event> event(static_cast<Event *>(data));
    if (!event || env == nullptr || callback == nullptr || event->job->closing) { return; }
    napi_value global = nullptr, json = nullptr, parse = nullptr, input = nullptr, object = nullptr;
    napi_value task_id = nullptr, ignored = nullptr;
    if (napi_get_global(env, &global) != napi_ok ||
        napi_get_named_property(env, global, "JSON", &json) != napi_ok ||
        napi_get_named_property(env, json, "parse", &parse) != napi_ok ||
        napi_create_string_utf8(env, event->json.data(), event->json.size(), &input) != napi_ok ||
        napi_call_function(env, json, parse, 1, &input, &object) != napi_ok ||
        napi_create_uint32(env, event->job->id, &task_id) != napi_ok ||
        napi_set_named_property(env, object, "taskId", task_id) != napi_ok) {
        return;
    }
    napi_call_function(env, global, callback, 1, &object, &ignored);
}

void OnEvent(const char *json, void *user) {
    auto *job = static_cast<Job *>(user);
    std::lock_guard<std::mutex> guard(job->delivery);
    if (job->closing || json == nullptr) { return; }
    try {
        auto event = std::make_unique<Event>(Event{job->shared_from_this(), json});
        if (napi_call_threadsafe_function(job->tsfn, event.get(), napi_tsfn_nonblocking) == napi_ok) {
            event.release();
        }
    } catch (...) {
        controller_probe_cancel(job->probe);
    }
}

napi_value Version(napi_env env, napi_callback_info) { return String(env, controller_version()); }

napi_value Validate(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value arg = nullptr;
    std::string input;
    if (napi_get_cb_info(env, info, &argc, &arg, nullptr, nullptr) != napi_ok || argc != 1 ||
        !ReadString(env, arg, input, 16384)) {
        Error(env, "INVALID_ARGUMENT", "Expected a bounded profile JSON string");
        return nullptr;
    }
    char *json = controller_validate_profile(input.c_str());
    napi_value result = String(env, json);
    controller_free_string(json);
    return result;
}

napi_value Start(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value args[3] = {};
    std::string endpoint;
    double timeout = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 3 ||
        !ReadString(env, args[0], endpoint, 256) ||
        napi_get_value_double(env, args[1], &timeout) != napi_ok ||
        !(timeout >= 100 && timeout <= 2000) || timeout != static_cast<uint32_t>(timeout) ||
        napi_typeof(env, args[2], &callback_type) != napi_ok || callback_type != napi_function) {
        Error(env, "INVALID_ARGUMENT", "Expected IP endpoint, timeout 100..2000 ms, and callback");
        return nullptr;
    }
    State *state = GetState(env);
    if (state == nullptr) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    if (state->workers->load() >= 4) {
        Error(env, "BUSY", "Previous network checks are still closing");
        return nullptr;
    }
    ControllerProbe *probe = controller_probe_create(endpoint.c_str(), static_cast<uint32_t>(timeout));
    if (!probe) { Error(env, "INVALID_ARGUMENT", "Expected a literal IP address and valid port"); return nullptr; }
    Close(state);
    auto job = std::make_shared<Job>();
    job->probe = probe;
    job->id = state->next_id++;
    if (job->id == 0) { job->id = state->next_id++; }
    napi_value name = nullptr, result = nullptr;
    auto *holder = new (std::nothrow) std::shared_ptr<Job>(job);
    if (holder == nullptr ||
        napi_create_string_utf8(env, "ControllerPreflight", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_uint32(env, job->id, &result) != napi_ok ||
        napi_create_threadsafe_function(env, args[2], nullptr, name, 16, 2, holder,
                                       Finalize, nullptr, CallJs, &job->tsfn) != napi_ok) {
        delete holder;
        Error(env, "NATIVE_FAILURE", "Unable to create event bridge");
        return nullptr;
    }
    auto count = state->workers;
    count->fetch_add(1);
    try {
        std::thread([job, count]() {
            controller_probe_run(job->probe, OnEvent, job.get());
            count->fetch_sub(1);
            napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        }).detach();
    } catch (...) {
        count->fetch_sub(1);
        job->closing = true;
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        Error(env, "NATIVE_FAILURE", "Unable to start network worker");
        return nullptr;
    }
    state->active = job;
    return result;
}

napi_value Cancel(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value arg = nullptr, result = nullptr;
    double id = 0;
    if (napi_get_cb_info(env, info, &argc, &arg, nullptr, nullptr) != napi_ok || argc != 1 ||
        napi_get_value_double(env, arg, &id) != napi_ok || !(id >= 1 && id <= UINT32_MAX) ||
        id != static_cast<uint32_t>(id)) {
        Error(env, "INVALID_ARGUMENT", "Expected a task ID"); return nullptr;
    }
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    bool found = state->active && state->active->id == static_cast<uint32_t>(id);
    if (found) { Close(state); }
    napi_get_boolean(env, found, &result);
    return result;
}

napi_value Dispose(napi_env env, napi_callback_info) {
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    Close(state);
    napi_value result = nullptr;
    napi_get_undefined(env, &result);
    return result;
}

napi_value Init(napi_env env, napi_value exports) {
    auto *state = new (std::nothrow) State;
    if (!state) { Error(env, "NATIVE_FAILURE", "Unable to allocate state"); return nullptr; }
    if (napi_set_instance_data(env, state, nullptr, nullptr) != napi_ok) {
        delete state; Error(env, "NATIVE_FAILURE", "Unable to initialize state"); return nullptr;
    }
    if (napi_add_env_cleanup_hook(env, Cleanup, state) != napi_ok) {
        napi_set_instance_data(env, nullptr, nullptr, nullptr);
        delete state; Error(env, "NATIVE_FAILURE", "Unable to register cleanup"); return nullptr;
    }
    napi_property_descriptor methods[] = {
        {"version", nullptr, Version, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"validateProfile", nullptr, Validate, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"probeEndpoint", nullptr, Start, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"cancel", nullptr, Cancel, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"dispose", nullptr, Dispose, nullptr, nullptr, nullptr, napi_default, nullptr},
    };
    if (napi_define_properties(env, exports, sizeof(methods) / sizeof(methods[0]), methods) != napi_ok) {
        napi_remove_env_cleanup_hook(env, Cleanup, state);
        napi_set_instance_data(env, nullptr, nullptr, nullptr);
        delete state; Error(env, "NATIVE_FAILURE", "Unable to export methods"); return nullptr;
    }
    return exports;
}
napi_module module = {1, 0, nullptr, Init, "controller", nullptr, {0}};
}

extern "C" __attribute__((constructor)) void RegisterControllerModule() { napi_module_register(&module); }
