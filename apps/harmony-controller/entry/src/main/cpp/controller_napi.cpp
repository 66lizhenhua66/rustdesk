#include <napi/native_api.h>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <memory>
#include <mutex>
#include <new>
#include <string>
#include <thread>

#include "controller.h"
#include "session.h"
#include "input.h"
#include "canvas.h"
#include "video_renderer.h"

namespace {
struct Job : std::enable_shared_from_this<Job> {
    uint32_t id = 0;
    ControllerProbe *probe = nullptr;
    ControllerSession *session = nullptr;
    ControllerInput *input = nullptr;
    ControllerCanvas *canvas = nullptr;
    bool persistent = false;
    std::unique_ptr<VideoRenderer> video;
    uint64_t notifiedVideoFrames = 0;
    napi_threadsafe_function tsfn = nullptr;
    std::atomic<bool> closing{false};
    std::mutex delivery;
    ~Job() {
        controller_input_free_v1(input);
        controller_canvas_free_v1(canvas);
        controller_probe_destroy(probe);
        controller_session_destroy(session);
    }
    void Cancel() {
        if (session != nullptr) { controller_session_cancel(session); }
        else { controller_probe_cancel(probe); }
        if (video) { video->Stop(); }
    }
};

struct Event {
    std::shared_ptr<Job> job;
    std::string json;
    ~Event() {
        volatile char *bytes = json.empty() ? nullptr : &json[0];
        for (size_t i = 0; i < json.size(); ++i) { bytes[i] = 0; }
    }
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
    job->Cancel();
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
    if (job->persistent) {
        if (json == nullptr) { return; }
        try {
            auto event = std::make_unique<Event>(Event{job->shared_from_this(), json});
            while (!job->closing) {
                {
                    std::lock_guard<std::mutex> guard(job->delivery);
                    if (job->closing) { return; }
                    const auto status = napi_call_threadsafe_function(job->tsfn, event.get(), napi_tsfn_nonblocking);
                    if (status == napi_ok) { event.release(); return; }
                    if (status != napi_queue_full) { job->Cancel(); return; }
                }
                // Preserve permission/close ordering without blocking the UI's cancellation lock.
                std::this_thread::sleep_for(std::chrono::milliseconds(10));
            }
        } catch (...) {
            job->Cancel();
        }
        return;
    }
    std::lock_guard<std::mutex> guard(job->delivery);
    if (job->closing || json == nullptr) { return; }
    try {
        auto event = std::make_unique<Event>(Event{job->shared_from_this(), json});
        if (napi_call_threadsafe_function(job->tsfn, event.get(), napi_tsfn_nonblocking) == napi_ok) {
            event.release();
        }
    } catch (...) {
        job->Cancel();
    }
}

napi_value Version(napi_env env, napi_callback_info) { return String(env, controller_version()); }

napi_value CreateIdentity(napi_env env, napi_callback_info) {
    char *json = controller_identity_create_v1();
    napi_value result = String(env, json);
    controller_secret_string_free_v1(json);
    return result;
}

void OnVideo(const uint8_t *data, uint32_t length, uint32_t width, uint32_t height,
             int64_t pts, uint8_t key_frame, void *user) {
    auto *job = static_cast<Job *>(user);
    if (job->closing || !job->video) { return; }
    try {
        if (!job->video->Submit(data, length, width, height, pts, key_frame != 0)) {
            if (!job->closing) {
                OnEvent("{\"state\":\"failed\",\"code\":\"VIDEO_RENDER_FAILED\",\"message\":\"Native video rendering failed\",\"verified\":true,\"authenticated\":false,\"authorized\":false}", user);
                job->Cancel();
            }
            return;
        }
        const auto count = job->video->PresentedFrames();
        if (count != job->notifiedVideoFrames && (count == 1 || (count > 0 && count % 8 == 0))) {
            job->notifiedVideoFrames = count;
            const auto json = std::string("{\"state\":\"video_rendered\",\"code\":\"VIDEO_RENDERED\",\"message\":\"Native frame presented\",\"verified\":true,\"authenticated\":true,\"authorized\":false,\"renderedFrames\":") + std::to_string(count) + "}";
            OnEvent(json.c_str(), user);
        }
    } catch (...) {
        job->Cancel();
    }
}

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

struct WipeString {
    std::string &value;
    ~WipeString() {
        volatile char *bytes = value.empty() ? nullptr : &value[0];
        for (size_t i = 0; i < value.size(); ++i) { bytes[i] = 0; }
    }
};

napi_value Authenticate(napi_env env, napi_callback_info info) {
    size_t argc = 4;
    napi_value args[4] = {};
    std::string request, password;
    WipeString wipe{password};
    double timeout = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 4 ||
        !ReadString(env, args[0], request, 16384) || !ReadString(env, args[1], password, 512) ||
        napi_get_value_double(env, args[2], &timeout) != napi_ok ||
        !(timeout >= 100 && timeout <= 30000) || timeout != static_cast<uint32_t>(timeout) ||
        napi_typeof(env, args[3], &callback_type) != napi_ok || callback_type != napi_function) {
        Error(env, "INVALID_ARGUMENT", "Expected trusted peer configuration, password, timeout, and callback");
        return nullptr;
    }
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    if (state->workers->load() >= 4) {
        Error(env, "BUSY", "Previous network operations are still closing"); return nullptr;
    }
    auto *session = controller_session_create(request.c_str(), password.c_str(), static_cast<uint32_t>(timeout));
    if (!session) { Error(env, "INVALID_TRUST_CONFIG", "Expected a pinned public key, peer ID, and valid IP endpoint"); return nullptr; }
    Close(state);
    auto job = std::make_shared<Job>();
    job->session = session;
    job->id = state->next_id++;
    if (job->id == 0) { job->id = state->next_id++; }
    napi_value name = nullptr, result = nullptr;
    auto *holder = new (std::nothrow) std::shared_ptr<Job>(job);
    if (holder == nullptr ||
        napi_create_string_utf8(env, "ControllerAuthentication", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_uint32(env, job->id, &result) != napi_ok ||
        napi_create_threadsafe_function(env, args[3], nullptr, name, 32, 2, holder,
                                       Finalize, nullptr, CallJs, &job->tsfn) != napi_ok) {
        delete holder;
        Error(env, "NATIVE_FAILURE", "Unable to create authentication event bridge"); return nullptr;
    }
    auto count = state->workers;
    count->fetch_add(1);
    try {
        std::thread([job, count]() {
            controller_session_run(job->session, OnEvent, job.get());
            count->fetch_sub(1);
            napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        }).detach();
    } catch (...) {
        count->fetch_sub(1);
        job->closing = true;
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        Error(env, "NATIVE_FAILURE", "Unable to start authentication worker"); return nullptr;
    }
    state->active = job;
    return result;
}

napi_value ConnectDemo(napi_env env, napi_callback_info info) {
    size_t argc = 4;
    napi_value args[4] = {};
    std::string request, password;
    WipeString wipe{password};
    double timeout = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 4 ||
        !ReadString(env, args[0], request, 16384) || !ReadString(env, args[1], password, 512) ||
        napi_get_value_double(env, args[2], &timeout) != napi_ok ||
        !(timeout >= 100 && timeout <= 60000) || timeout != static_cast<uint32_t>(timeout) ||
        napi_typeof(env, args[3], &callback_type) != napi_ok || callback_type != napi_function) {
        Error(env, "INVALID_ARGUMENT", "Expected trusted peer configuration, password, timeout, and callback");
        return nullptr;
    }
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    if (state->workers->load() >= 4) {
        Error(env, "BUSY", "Previous network operations are still closing"); return nullptr;
    }
    auto *session = controller_connection_create(request.c_str(), password.c_str(), static_cast<uint32_t>(timeout));
    if (!session) { Error(env, "INVALID_TRUST_CONFIG", "Expected a pinned public key, peer ID, and valid IP endpoint"); return nullptr; }
    Close(state);
    auto job = std::make_shared<Job>();
    job->persistent = true;
    job->session = session;
    job->id = state->next_id++;
    if (job->id == 0) { job->id = state->next_id++; }
    napi_value name = nullptr, result = nullptr;
    auto *holder = new (std::nothrow) std::shared_ptr<Job>(job);
    if (holder == nullptr ||
        napi_create_string_utf8(env, "ControllerDemoConnection", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_uint32(env, job->id, &result) != napi_ok ||
        napi_create_threadsafe_function(env, args[3], nullptr, name, 32, 2, holder,
                                       Finalize, nullptr, CallJs, &job->tsfn) != napi_ok) {
        delete holder;
        Error(env, "NATIVE_FAILURE", "Unable to create authentication event bridge"); return nullptr;
    }
    auto count = state->workers;
    count->fetch_add(1);
    try {
        std::thread([job, count]() {
            controller_session_run(job->session, OnEvent, job.get());
            count->fetch_sub(1);
            napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        }).detach();
    } catch (...) {
        count->fetch_sub(1);
        job->closing = true;
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        Error(env, "NATIVE_FAILURE", "Unable to start authentication worker"); return nullptr;
    }
    state->active = job;
    return result;
}

napi_value ConnectScreen(napi_env env, napi_callback_info info) {
    size_t argc = 4;
    napi_value args[4] = {};
    std::string request, surface;
    double timeout = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 4 ||
        !ReadString(env, args[0], request, 16384) || !ReadString(env, args[1], surface, 32) ||
        napi_get_value_double(env, args[2], &timeout) != napi_ok ||
        !(timeout >= 100 && timeout <= 60000) || timeout != static_cast<uint32_t>(timeout) ||
        napi_typeof(env, args[3], &callback_type) != napi_ok || callback_type != napi_function) {
        Error(env, "INVALID_ARGUMENT", "Expected trusted screen configuration, surface ID, timeout, and callback");
        return nullptr;
    }
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    if (state->workers->load() >= 4) {
        Error(env, "BUSY", "Previous network operations are still closing"); return nullptr;
    }
    auto *session = controller_connection_create(request.c_str(), "", static_cast<uint32_t>(timeout));
    if (!session) { Error(env, "INVALID_TRUST_CONFIG", "Expected a pinned public key, peer ID, and valid IP endpoint"); return nullptr; }
    auto renderer = std::make_unique<VideoRenderer>(surface);
    if (!renderer->Ready()) {
        controller_session_destroy(session);
        Error(env, "SURFACE_UNAVAILABLE", "The native screen surface is not ready"); return nullptr;
    }
    Close(state);
    auto job = std::make_shared<Job>();
    job->persistent = true;
    job->session = session;
    job->video = std::move(renderer);
    job->id = state->next_id++;
    if (job->id == 0) { job->id = state->next_id++; }
    napi_value name = nullptr, result = nullptr;
    auto *holder = new (std::nothrow) std::shared_ptr<Job>(job);
    if (holder == nullptr ||
        napi_create_string_utf8(env, "ControllerScreenConnection", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_uint32(env, job->id, &result) != napi_ok ||
        napi_create_threadsafe_function(env, args[3], nullptr, name, 32, 2, holder,
                                       Finalize, nullptr, CallJs, &job->tsfn) != napi_ok) {
        delete holder;
        Error(env, "NATIVE_FAILURE", "Unable to create authentication event bridge"); return nullptr;
    }
    auto count = state->workers;
    count->fetch_add(1);
    try {
        std::thread([job, count]() {
            controller_session_run_video(job->session, OnEvent, OnVideo, job.get());
            job->video->Stop();
            count->fetch_sub(1);
            napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        }).detach();
    } catch (...) {
        count->fetch_sub(1);
        job->closing = true;
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        Error(env, "NATIVE_FAILURE", "Unable to start authentication worker"); return nullptr;
    }
    state->active = job;
    return result;
}

napi_value ConnectTrustedScreen(napi_env env, napi_callback_info info) {
    size_t argc = 5;
    napi_value args[5] = {};
    std::string request, credentials, surface;
    WipeString wipe{credentials};
    double timeout = 0;
    napi_valuetype callback_type;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 5 ||
        !ReadString(env, args[0], request, 16384) || !ReadString(env, args[1], credentials, 2048) ||
        !ReadString(env, args[2], surface, 32) ||
        napi_get_value_double(env, args[3], &timeout) != napi_ok ||
        !(timeout >= 100 && timeout <= 60000) || timeout != static_cast<uint32_t>(timeout) ||
        napi_typeof(env, args[4], &callback_type) != napi_ok || callback_type != napi_function) {
        Error(env, "INVALID_ARGUMENT", "Expected trusted request, credentials, surface ID, timeout, and callback");
        return nullptr;
    }
    auto *state = GetState(env);
    if (!state) { Error(env, "NATIVE_FAILURE", "Controller unavailable"); return nullptr; }
    if (state->workers->load() >= 4) {
        Error(env, "BUSY", "Previous network operations are still closing"); return nullptr;
    }
    auto *session = controller_connection_create_access_v1(request.c_str(), credentials.c_str(),
                                                            static_cast<uint32_t>(timeout));
    if (!session) { Error(env, "INVALID_TRUST_CONFIG", "Invalid pinned target or credential"); return nullptr; }
    auto renderer = std::make_unique<VideoRenderer>(surface);
    if (!renderer->Ready()) {
        controller_session_destroy(session);
        Error(env, "SURFACE_UNAVAILABLE", "The native screen surface is not ready"); return nullptr;
    }
    Close(state);
    auto job = std::make_shared<Job>();
    job->persistent = true;
    job->session = session;
    job->video = std::move(renderer);
    job->id = state->next_id++;
    if (job->id == 0) { job->id = state->next_id++; }
    napi_value name = nullptr, result = nullptr;
    auto *holder = new (std::nothrow) std::shared_ptr<Job>(job);
    if (holder == nullptr ||
        napi_create_string_utf8(env, "ControllerTrustedScreenConnection", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_uint32(env, job->id, &result) != napi_ok ||
        napi_create_threadsafe_function(env, args[4], nullptr, name, 32, 2, holder,
                                       Finalize, nullptr, CallJs, &job->tsfn) != napi_ok) {
        delete holder;
        Error(env, "NATIVE_FAILURE", "Unable to create trusted event bridge"); return nullptr;
    }
    auto count = state->workers;
    count->fetch_add(1);
    try {
        std::thread([job, count]() {
            controller_session_run_video(job->session, OnEvent, OnVideo, job.get());
            job->video->Stop();
            count->fetch_sub(1);
            napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        }).detach();
    } catch (...) {
        count->fetch_sub(1);
        job->closing = true;
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(job->tsfn, napi_tsfn_release);
        Error(env, "NATIVE_FAILURE", "Unable to start trusted worker"); return nullptr;
    }
    state->active = job;
    return result;
}

bool ReadBoundedInteger(napi_env env, napi_value value, uint32_t minimum, uint32_t maximum, uint32_t &out) {
    double number = 0;
    if (napi_get_value_double(env, value, &number) != napi_ok ||
        !(number >= minimum && number <= maximum) || number != static_cast<uint32_t>(number)) { return false; }
    out = static_cast<uint32_t>(number);
    return true;
}

std::shared_ptr<Job> ActiveSession(napi_env env, uint32_t id) {
    auto *state = GetState(env);
    if (!state || !state->active || state->active->id != id || state->active->closing ||
        !state->active->session) { return nullptr; }
    return state->active;
}

napi_value SendPointer(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value args[3] = {}, result = nullptr;
    uint32_t id = 0, x = 0, y = 0;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 3 ||
        !ReadBoundedInteger(env, args[0], 1, UINT32_MAX, id) ||
        !ReadBoundedInteger(env, args[1], 0, 799, x) || !ReadBoundedInteger(env, args[2], 0, 449, y)) {
        Error(env, "INVALID_ARGUMENT", "Expected task ID and demo coordinates 0..799, 0..449"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    bool queued = job && controller_session_send_pointer(job->session, x, y) == 0;
    napi_get_boolean(env, queued, &result);
    return result;
}

napi_value SendText(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {}, result = nullptr;
    uint32_t id = 0;
    std::string text;
    WipeString wipe{text};
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 2 ||
        !ReadBoundedInteger(env, args[0], 1, UINT32_MAX, id) || !ReadString(env, args[1], text, 512) || text.empty()) {
        Error(env, "INVALID_ARGUMENT", "Expected task ID and nonempty text up to 512 UTF-8 bytes"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    bool queued = job && controller_session_send_text(job->session, text.c_str()) == 0;
    napi_get_boolean(env, queued, &result);
    return result;
}

napi_value SendSystemInput(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {}, result = nullptr;
    uint32_t id = 0;
    std::string command;
    WipeString wipe{command};
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 2 ||
        !ReadBoundedInteger(env, args[0], 1, UINT32_MAX, id) ||
        !ReadString(env, args[1], command, 4096) || command.empty()) {
        Error(env, "INVALID_ARGUMENT", "Expected task ID and bounded input command JSON"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    const int32_t status = job ? controller_session_send_input_v1(job->session, command.c_str()) : 1;
    napi_create_int32(env, status, &result);
    return result;
}

napi_value SetInputEnabled(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {}, result = nullptr;
    uint32_t id = 0;
    bool enabled = false;
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 2 ||
        !ReadBoundedInteger(env, args[0], 1, UINT32_MAX, id) ||
        napi_get_value_bool(env, args[1], &enabled) != napi_ok) {
        Error(env, "INVALID_ARGUMENT", "Expected task ID and input enable boolean"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    if (job && !enabled) {
        controller_input_reset_v1(job->input);
        controller_canvas_free_v1(job->canvas);
        job->canvas = nullptr;
    }
    const int32_t status = job ? controller_session_set_input_enabled_v1(job->session, static_cast<uint8_t>(enabled)) : 1;
    napi_create_int32(env, status, &result);
    return result;
}

int32_t QueueSystemInput(void *context, const char *command) {
    return controller_session_send_input_v1(static_cast<ControllerSession *>(context), command);
}

int32_t QueueCanvasInput(void *context, const char *command) {
    return controller_session_send_input_v1(static_cast<ControllerSession *>(context), command);
}

napi_value SendSystemInputEvent(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {}, result = nullptr;
    uint32_t id = 0;
    std::string event;
    WipeString wipe{event};
    if (napi_get_cb_info(env, info, &argc, args, nullptr, nullptr) != napi_ok || argc != 2 ||
        !ReadBoundedInteger(env, args[0], 1, UINT32_MAX, id) ||
        !ReadString(env, args[1], event, 8192) || event.empty()) {
        Error(env, "INVALID_ARGUMENT", "Expected task ID and bounded standard input event JSON"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    int32_t status = 1;
    if (job) {
        if (!job->canvas) { job->canvas = controller_canvas_new_v1(); }
        if (job->canvas) {
            status = controller_canvas_event_v1(job->canvas, event.c_str(), QueueCanvasInput, job->session);
        }
        if (status != 0) {
            controller_canvas_free_v1(job->canvas);
            job->canvas = nullptr;
            if (controller_session_set_input_enabled_v1(job->session, 0) != 0) {
                controller_session_cancel(job->session);
            }
        }
    }
    napi_create_int32(env, status, &result);
    return result;
}

napi_value ResetSystemInput(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value arg = nullptr, result = nullptr;
    uint32_t id = 0;
    if (napi_get_cb_info(env, info, &argc, &arg, nullptr, nullptr) != napi_ok || argc != 1 ||
        !ReadBoundedInteger(env, arg, 1, UINT32_MAX, id)) {
        Error(env, "INVALID_ARGUMENT", "Expected a task ID"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    if (job) { controller_input_reset_v1(job->input); }
    napi_get_undefined(env, &result);
    return result;
}

napi_value CanvasState(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value arg = nullptr;
    uint32_t id = 0;
    if (napi_get_cb_info(env, info, &argc, &arg, nullptr, nullptr) != napi_ok || argc != 1 ||
        !ReadBoundedInteger(env, arg, 1, UINT32_MAX, id)) {
        Error(env, "INVALID_ARGUMENT", "Expected a task ID"); return nullptr;
    }
    auto job = ActiveSession(env, id);
    if (!job || !job->canvas) { return String(env, "{}"); }
    char *state = controller_canvas_state_v1(job->canvas);
    napi_value result = String(env, state);
    controller_free_string(state);
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
        {"createIdentity", nullptr, CreateIdentity, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"validateProfile", nullptr, Validate, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"probeEndpoint", nullptr, Start, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"authenticate", nullptr, Authenticate, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"connectDemo", nullptr, ConnectDemo, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"connectScreen", nullptr, ConnectScreen, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"connectTrustedScreen", nullptr, ConnectTrustedScreen, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"sendPointer", nullptr, SendPointer, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"sendText", nullptr, SendText, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"sendInput", nullptr, SendSystemInput, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"sendInputEvent", nullptr, SendSystemInputEvent, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"resetInput", nullptr, ResetSystemInput, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"canvasState", nullptr, CanvasState, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"setInputEnabled", nullptr, SetInputEnabled, nullptr, nullptr, nullptr, napi_default, nullptr},
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
