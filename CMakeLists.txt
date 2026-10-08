# SPDX-License-Identifier: Apache-2.0
#
# CMake front end for the Rust workspace: it calls `cargo`, it compiles nothing itself.
#
#   cmake -S . -B build
#   cmake --build build --target usage-cockpit   # cargo build --release
#   cmake --build build --target check           # cargo fmt --check and cargo clippy -D warnings
#   ctest --test-dir build -C Release            # fmt, clippy and the tests of the workspace
#   cmake --build build --target dist            # dist/usage-cockpit-<version>-<system>[.exe] + SHA256SUMS
#
# Options:
#   -DCOCKPIT_TARGET=<rust target triple>   build for another target (default: the host of rustc)
#   -DCOCKPIT_CARGO=<path to cargo>         use this cargo
#
# The build output of cargo goes to <build dir>/cargo-target, so a CMake build never mixes with
# the `target/` folder of plain `cargo` runs.

cmake_minimum_required(VERSION 3.21)

# Never build inside the source tree: CMake's files and the output of cargo would land in it.
if("${CMAKE_BINARY_DIR}" STREQUAL "${CMAKE_SOURCE_DIR}")
  message(FATAL_ERROR "Use a build folder of its own, for example: cmake -S . -B build")
endif()

# The version is the one of the workspace: the `version = "x.y.z"` line of the table
# [workspace.package] in Cargo.toml (the text up to the next table).
file(READ "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" _cargo_toml)
string(REGEX MATCH "\\[workspace\\.package\\][^\\[]*" _workspace_package "${_cargo_toml}")
string(REGEX MATCH "(^|\n)version[ \t]*=[ \t]*\"([0-9]+\\.[0-9]+\\.[0-9]+[^\"]*)\"" _version_line "${_workspace_package}")
set(COCKPIT_VERSION "${CMAKE_MATCH_2}")
if(COCKPIT_VERSION STREQUAL "")
  message(FATAL_ERROR "The version could not be read from the table [workspace.package] of Cargo.toml")
endif()
string(REGEX MATCH "^[0-9]+\\.[0-9]+\\.[0-9]+" _cmake_version "${COCKPIT_VERSION}")
# A new version in Cargo.toml makes CMake configure again, so the name of the dist file follows.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml")

# No language: nothing is compiled by CMake, so no C/C++ compiler is looked for.
project(usage-cockpit VERSION ${_cmake_version} LANGUAGES NONE)

set(COCKPIT_CARGO "" CACHE FILEPATH "cargo to use (default: found on the PATH or in the cargo bin folder)")
set(COCKPIT_TARGET "" CACHE STRING "Rust target triple (default: the host of rustc)")

# ---------------------------------------------------------------- cargo and rustc
# The cargo bin folder of rustup is looked at first (its `cargo` honours rust-toolchain.toml), then
# the PATH. Only folders from variables that are set are used. NO_CACHE: a changed option or PATH
# takes effect at the next configure.
unset(_cargo)
unset(_rustc)
set(_cargo_hints)
foreach(_home "$ENV{CARGO_HOME}" "$ENV{USERPROFILE}/.cargo" "$ENV{HOME}/.cargo")
  if(NOT "${_home}" STREQUAL "" AND NOT "${_home}" STREQUAL "/.cargo")
    list(APPEND _cargo_hints "${_home}/bin")
  endif()
endforeach()
if(COCKPIT_CARGO)
  set(_cargo "${COCKPIT_CARGO}")
else()
  find_program(_cargo cargo HINTS ${_cargo_hints} NO_CACHE)
endif()
if(NOT _cargo)
  message(FATAL_ERROR "cargo was not found. Install the tools first (see docs/development-guide.md) or pass -DCOCKPIT_CARGO=<path>.")
endif()
get_filename_component(_cargo_dir "${_cargo}" DIRECTORY)
find_program(_rustc rustc HINTS "${_cargo_dir}" NO_DEFAULT_PATH NO_CACHE)
if(NOT _rustc)
  find_program(_rustc rustc HINTS ${_cargo_hints} NO_CACHE)
endif()
if(NOT _rustc)
  message(FATAL_ERROR "rustc was not found next to cargo (${_cargo_dir}).")
endif()

# The host of rustc, run in the source dir so that the pinned toolchain of rust-toolchain.toml is
# the one asked. (With the rustup `rustc` this can download that toolchain at configure time.)
execute_process(COMMAND "${_rustc}" -vV
                WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
                OUTPUT_VARIABLE _rustc_info RESULT_VARIABLE _rustc_result
                OUTPUT_STRIP_TRAILING_WHITESPACE)
if(NOT _rustc_result EQUAL 0)
  message(FATAL_ERROR "rustc -vV failed (${_rustc_result}); is the toolchain of rust-toolchain.toml installed?")
endif()
string(REGEX MATCH "host: ([^\r\n]+)" _unused "${_rustc_info}")
set(_host_triple "${CMAKE_MATCH_1}")

# The target triple: given, or the host.
if(COCKPIT_TARGET)
  set(_triple "${COCKPIT_TARGET}")
else()
  set(_triple "${_host_triple}")
endif()

# The name of the system in the file name of a release (see the user guide).
set(_known_triples
    "x86_64-pc-windows-msvc=windows-x64"
    "x86_64-unknown-linux-gnu=linux-x64"
    "aarch64-unknown-linux-gnu=linux-arm64"
    "aarch64-apple-darwin=macos-arm64")
set(COCKPIT_SYSTEM "")
foreach(_pair IN LISTS _known_triples)
  string(REPLACE "=" ";" _kv "${_pair}")
  list(GET _kv 0 _key)
  list(GET _kv 1 _value)
  if(_key STREQUAL _triple)
    set(COCKPIT_SYSTEM "${_value}")
  endif()
endforeach()
if(NOT COCKPIT_SYSTEM)
  message(FATAL_ERROR "The target '${_triple}' is not one of the supported targets (${_known_triples}).")
endif()

if(COCKPIT_TARGET)
  set(_target_args --target "${_triple}")
  set(_release_dir "${CMAKE_BINARY_DIR}/cargo-target/${_triple}/release")
else()
  set(_target_args)
  set(_release_dir "${CMAKE_BINARY_DIR}/cargo-target/release")
endif()
if(_triple MATCHES "windows")
  set(_exe_suffix ".exe")
else()
  set(_exe_suffix "")
endif()
set(COCKPIT_EXE "${_release_dir}/usage-cockpit${_exe_suffix}")
set(COCKPIT_DIST_NAME "usage-cockpit-${COCKPIT_VERSION}-${COCKPIT_SYSTEM}${_exe_suffix}")

message(STATUS "usage-cockpit ${COCKPIT_VERSION}, target ${_triple} (${COCKPIT_SYSTEM})")
message(STATUS "cargo: ${_cargo}")

# `cargo` run in the source dir, output in the build dir.
set(_cargo_run ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/cargo-target" "${_cargo}")

# ---------------------------------------------------------------- targets
add_custom_target(usage-cockpit ALL
  COMMAND ${_cargo_run} build --release --locked ${_target_args}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  COMMENT "cargo build --release (${_triple})"
  USES_TERMINAL VERBATIM)

add_custom_target(check
  COMMAND ${_cargo_run} fmt --all --check
  COMMAND ${_cargo_run} clippy --all-targets --locked ${_target_args} -- -D warnings
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  COMMENT "cargo fmt --check and cargo clippy -D warnings"
  USES_TERMINAL VERBATIM)

# The tests are run by CTest (`ctest --test-dir build -C Release`, or the `test` target that
# CMake makes of it).
enable_testing()
add_test(NAME cargo-fmt
  COMMAND ${_cargo_run} fmt --all --check
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")
add_test(NAME cargo-clippy
  COMMAND ${_cargo_run} clippy --all-targets --locked ${_target_args} -- -D warnings
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")
# The tests of a target that is not the host cannot be run here: they are only compiled.
if(_triple STREQUAL _host_triple)
  set(_test_args)
else()
  set(_test_args --no-run)
  message(STATUS "target ${_triple} is not the host (${_host_triple}): cargo-test only compiles the tests")
endif()
add_test(NAME cargo-test
  COMMAND ${_cargo_run} test --all --locked ${_target_args} ${_test_args}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")

# dist/<name> and dist/SHA256SUMS
add_custom_target(dist
  COMMAND ${CMAKE_COMMAND}
          "-DSRC=${COCKPIT_EXE}"
          "-DDEST_DIR=${CMAKE_BINARY_DIR}/dist"
          "-DNAME=${COCKPIT_DIST_NAME}"
          -P "${CMAKE_CURRENT_SOURCE_DIR}/cmake/dist.cmake"
  COMMENT "dist: ${COCKPIT_DIST_NAME}"
  USES_TERMINAL VERBATIM)
add_dependencies(dist usage-cockpit)
