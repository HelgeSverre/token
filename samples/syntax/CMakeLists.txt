# CMake Syntax Highlighting Test
# A cross-platform C++ project with dependencies, testing, and packaging.

cmake_minimum_required(VERSION 3.25 FATAL_ERROR)

# Project definition
project(TokenEditor
    VERSION 0.3.19
    DESCRIPTION "A minimal text editor implementing the Elm Architecture"
    HOMEPAGE_URL "https://github.com/example/token-editor"
    LANGUAGES CXX C
)

# ============================================================
# Options and configuration
# ============================================================

option(TOKEN_BUILD_TESTS "Build test suite" ON)
option(TOKEN_BUILD_BENCHMARKS "Build benchmarks" OFF)
option(TOKEN_ENABLE_ASAN "Enable AddressSanitizer" OFF)
option(TOKEN_ENABLE_LTO "Enable link-time optimization" OFF)
option(TOKEN_USE_SYSTEM_LIBS "Use system-installed libraries" OFF)
option(TOKEN_INSTALL "Generate install target" ON)

set(TOKEN_FONT_DIR "${CMAKE_SOURCE_DIR}/assets" CACHE PATH "Font directory")
set(TOKEN_THEME_DIR "${CMAKE_SOURCE_DIR}/themes" CACHE PATH "Theme directory")

# C++ standard
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Build type defaults
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    message(STATUS "Setting build type to 'Release' as none was specified")
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
    set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS
        "Debug" "Release" "MinSizeRel" "RelWithDebInfo")
endif()

# ============================================================
# Platform detection and compiler setup
# ============================================================

message(STATUS "System: ${CMAKE_SYSTEM_NAME} ${CMAKE_SYSTEM_PROCESSOR}")
message(STATUS "Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")

# Compiler warnings
add_compile_options(
    $<$<CXX_COMPILER_ID:GNU,Clang,AppleClang>:-Wall -Wextra -Wpedantic>
    $<$<CXX_COMPILER_ID:GNU,Clang,AppleClang>:-Wno-unused-parameter>
    $<$<CXX_COMPILER_ID:GNU>:-Wno-maybe-uninitialized>
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /permissive->
)

# Sanitizers
if(TOKEN_ENABLE_ASAN)
    add_compile_options(-fsanitize=address -fno-omit-frame-pointer)
    add_link_options(-fsanitize=address)
    message(STATUS "AddressSanitizer enabled")
endif()

# LTO
if(TOKEN_ENABLE_LTO)
    include(CheckIPOSupported)
    check_ipo_supported(RESULT lto_supported OUTPUT lto_error)
    if(lto_supported)
        set(CMAKE_INTERPROCEDURAL_OPTIMIZATION ON)
        message(STATUS "LTO enabled")
    else()
        message(WARNING "LTO not supported: ${lto_error}")
    endif()
endif()

# ============================================================
# Dependencies
# ============================================================

include(FetchContent)

# Allow offline builds with pre-fetched deps
set(FETCHCONTENT_QUIET OFF)

if(TOKEN_USE_SYSTEM_LIBS)
    find_package(fmt REQUIRED)
    find_package(spdlog REQUIRED)
else()
    FetchContent_Declare(fmt
        GIT_REPOSITORY https://github.com/fmtlib/fmt.git
        GIT_TAG 10.2.1
        GIT_SHALLOW ON
    )

    FetchContent_Declare(spdlog
        GIT_REPOSITORY https://github.com/gabime/spdlog.git
        GIT_TAG v1.13.0
        GIT_SHALLOW ON
    )

    FetchContent_MakeAvailable(fmt spdlog)
endif()

# Platform-specific graphics
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(WAYLAND IMPORTED_TARGET wayland-client wayland-egl)
    pkg_check_modules(XKB IMPORTED_TARGET xkbcommon)
    pkg_check_modules(FONTCONFIG IMPORTED_TARGET fontconfig)
    pkg_check_modules(FREETYPE IMPORTED_TARGET freetype2)

    if(WAYLAND_FOUND)
        message(STATUS "Wayland support: enabled")
    else()
        message(STATUS "Wayland support: disabled (missing dependencies)")
    endif()
elseif(APPLE)
    find_library(COCOA_FRAMEWORK Cocoa REQUIRED)
    find_library(METAL_FRAMEWORK Metal REQUIRED)
    find_library(QUARTZCORE_FRAMEWORK QuartzCore REQUIRED)
    find_library(CORETEXT_FRAMEWORK CoreText REQUIRED)
endif()

# ============================================================
# Main library
# ============================================================

# Collect sources with glob (with CONFIGURE_DEPENDS for dev)
file(GLOB_RECURSE LIB_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/src/*.cpp"
    "${CMAKE_SOURCE_DIR}/src/*.h"
)

# Remove main.cpp from library sources
list(FILTER LIB_SOURCES EXCLUDE REGEX "main\\.cpp$")

add_library(token_lib STATIC ${LIB_SOURCES})

target_include_directories(token_lib
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/src>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_BINARY_DIR}/generated
)

target_link_libraries(token_lib
    PUBLIC
        fmt::fmt
        spdlog::spdlog
)

# Platform libraries
if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND WAYLAND_FOUND)
    target_link_libraries(token_lib PUBLIC
        PkgConfig::WAYLAND
        PkgConfig::XKB
        PkgConfig::FONTCONFIG
        PkgConfig::FREETYPE
    )
    target_compile_definitions(token_lib PUBLIC TOKEN_PLATFORM_LINUX)
elseif(APPLE)
    target_link_libraries(token_lib PUBLIC
        ${COCOA_FRAMEWORK}
        ${METAL_FRAMEWORK}
        ${QUARTZCORE_FRAMEWORK}
        ${CORETEXT_FRAMEWORK}
    )
    target_compile_definitions(token_lib PUBLIC TOKEN_PLATFORM_MACOS)
elseif(WIN32)
    target_link_libraries(token_lib PUBLIC d2d1 dwrite shcore)
    target_compile_definitions(token_lib PUBLIC TOKEN_PLATFORM_WINDOWS)
endif()

# Generate version header
configure_file(
    "${CMAKE_SOURCE_DIR}/src/version.h.in"
    "${CMAKE_BINARY_DIR}/generated/version.h"
    @ONLY
)

# ============================================================
# Executable
# ============================================================

add_executable(token src/main.cpp)
target_link_libraries(token PRIVATE token_lib)

# Embed assets path
target_compile_definitions(token PRIVATE
    TOKEN_FONT_DIR="${TOKEN_FONT_DIR}"
    TOKEN_THEME_DIR="${TOKEN_THEME_DIR}"
    TOKEN_VERSION="${PROJECT_VERSION}"
)

# ============================================================
# Tests
# ============================================================

if(TOKEN_BUILD_TESTS)
    enable_testing()

    FetchContent_Declare(Catch2
        GIT_REPOSITORY https://github.com/catchorg/Catch2.git
        GIT_TAG v3.5.2
        GIT_SHALLOW ON
    )
    FetchContent_MakeAvailable(Catch2)

    file(GLOB_RECURSE TEST_SOURCES CONFIGURE_DEPENDS
        "${CMAKE_SOURCE_DIR}/tests/*.cpp"
    )

    add_executable(token_tests ${TEST_SOURCES})
    target_link_libraries(token_tests PRIVATE
        token_lib
        Catch2::Catch2WithMain
    )

    include(Catch)
    catch_discover_tests(token_tests
        WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
        PROPERTIES
            TIMEOUT 30
            LABELS "unit"
    )

    # Custom test target with nice output
    add_custom_target(check
        COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -j ${CMAKE_BUILD_PARALLEL_LEVEL}
        DEPENDS token_tests
        COMMENT "Running tests..."
    )
endif()

# ============================================================
# Install and packaging
# ============================================================

if(TOKEN_INSTALL)
    include(GNUInstallDirs)

    install(TARGETS token
        RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
    )

    install(DIRECTORY assets/
        DESTINATION ${CMAKE_INSTALL_DATADIR}/token-editor/assets
    )

    install(DIRECTORY themes/
        DESTINATION ${CMAKE_INSTALL_DATADIR}/token-editor/themes
        FILES_MATCHING PATTERN "*.yaml"
    )

    # CPack configuration
    set(CPACK_PACKAGE_NAME "token-editor")
    set(CPACK_PACKAGE_VENDOR "Token Editor Contributors")
    set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "${PROJECT_DESCRIPTION}")
    set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
    set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/LICENSE")

    if(APPLE)
        set(CPACK_GENERATOR "DragNDrop")
        set(CPACK_DMG_VOLUME_NAME "Token Editor")
    elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
        set(CPACK_GENERATOR "DEB;RPM;TGZ")
        set(CPACK_DEBIAN_PACKAGE_DEPENDS "libfontconfig1, libfreetype6")
        set(CPACK_DEBIAN_PACKAGE_MAINTAINER "maintainer@example.com")
        set(CPACK_RPM_PACKAGE_LICENSE "MIT")
    elseif(WIN32)
        set(CPACK_GENERATOR "NSIS;ZIP")
        set(CPACK_NSIS_DISPLAY_NAME "Token Editor")
        set(CPACK_NSIS_INSTALL_ROOT "$PROGRAMFILES64")
    endif()

    include(CPack)
endif()

# ============================================================
# Print configuration summary
# ============================================================

message(STATUS "")
message(STATUS "=== Token Editor ${PROJECT_VERSION} Configuration ===")
message(STATUS "  Build type:    ${CMAKE_BUILD_TYPE}")
message(STATUS "  Tests:         ${TOKEN_BUILD_TESTS}")
message(STATUS "  Benchmarks:    ${TOKEN_BUILD_BENCHMARKS}")
message(STATUS "  ASAN:          ${TOKEN_ENABLE_ASAN}")
message(STATUS "  LTO:           ${TOKEN_ENABLE_LTO}")
message(STATUS "  Install:       ${TOKEN_INSTALL}")
message(STATUS "  System libs:   ${TOKEN_USE_SYSTEM_LIBS}")
message(STATUS "")
