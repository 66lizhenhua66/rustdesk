cmake_minimum_required(VERSION 3.5.0)
project(harmony_probe)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(OHOS_ARCH STREQUAL "x86_64")
    set(RUST_TARGET "x86_64-unknown-linux-ohos")
elseif(OHOS_ARCH STREQUAL "arm64" OR OHOS_ARCH STREQUAL "arm64-v8a")
    set(RUST_TARGET "aarch64-unknown-linux-ohos")
else()
    message(FATAL_ERROR "Unsupported OHOS_ARCH: ${OHOS_ARCH}")
endif()

set(RUST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../../native/rust")
set(RUST_LIB "${RUST_DIR}/target/${RUST_TARGET}/release/libharmony_probe.a")
if(NOT EXISTS "${RUST_LIB}")
    message(FATAL_ERROR "Build Rust staticlib first: ${RUST_LIB}")
endif()

add_library(probe SHARED probe_napi.cpp)
target_include_directories(probe PRIVATE "${RUST_DIR}/include")
target_link_libraries(probe PRIVATE "${RUST_LIB}" libace_napi.z.so)
