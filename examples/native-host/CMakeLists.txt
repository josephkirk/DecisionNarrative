cmake_minimum_required(VERSION 3.20)
project(storylet_native_host LANGUAGES CXX)

if(NOT WIN32)
  message(FATAL_ERROR "This sample CMake file currently targets the Windows Rust DLL output.")
endif()

set(STORYLET_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "DecisionNarrative repository root")
set(STORYLET_TARGET_DIR "${STORYLET_ROOT}/crates/storylet-engine/target/debug" CACHE PATH "Cargo target directory")

add_executable(storylet_native_host main.cpp)
target_compile_features(storylet_native_host PRIVATE cxx_std_17)
target_include_directories(storylet_native_host PRIVATE "${STORYLET_ROOT}/crates/storylet-engine/include")
target_link_libraries(storylet_native_host PRIVATE "${STORYLET_TARGET_DIR}/storylet_engine.dll.lib")
target_compile_definitions(storylet_native_host PRIVATE
  STORYLET_PROJECT_PATH="${CMAKE_CURRENT_SOURCE_DIR}/storylet-project.json")
add_custom_command(TARGET storylet_native_host POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
    "${STORYLET_TARGET_DIR}/storylet_engine.dll"
    "$<TARGET_FILE_DIR:storylet_native_host>/storylet_engine.dll")
