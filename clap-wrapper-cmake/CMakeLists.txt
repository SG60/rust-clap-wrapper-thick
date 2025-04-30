cmake_minimum_required(VERSION 3.15)
project(rust-plugin-1) # <-- change me

set(CMAKE_OSX_DEPLOYMENT_TARGET 15.4)

set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# the following lines are required to build AU SDK on macOS
set(CMAKE_CXX_STANDARD 17)
enable_language(OBJC)
enable_language(OBJCXX)
set(CMAKE_OBJC_VISIBILITY_PRESET hidden)
set(CMAKE_OBJCXX_VISIBILITY_PRESET hidden)
set(CMAKE_VISIBILITY_INLINES_HIDDEN ON)

set(BUILD_SHARED_LIBS OFF CACHE BOOL "Never want shared if not specified")

# ensure clap-wrapper lives as a submodule next to this CMakeLists.txt
# https://github.com/free-audio/clap-wrapper/
set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES TRUE CACHE BOOL "Get em")
set(CLAP_WRAPPER_DONT_ADD_TARGETS TRUE CACHE BOOL "I'll targetize")
set(CLAP_WRAPPER_BUILD_AUV2 TRUE CACHE BOOL "It's only logical")

add_subdirectory(clap-wrapper)

############################################################
# Rust import setup - corrosion
include(FetchContent)

FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.5 # Optionally specify a commit hash, version tag or branch here
)
# Set any global configuration variables such as `Rust_TOOLCHAIN` before this line!
FetchContent_MakeAvailable(Corrosion)

# Import targets defined in a package or workspace manifest `Cargo.toml` file
corrosion_import_crate(MANIFEST_PATH ../Cargo.toml)

############################################################
# SETUP FOR THICK CLAP WRAPPING
# This includes all the CLAP stuff inside the audio unit statically,
# rather than linking to a separate CLAP plugin dynamically.

# Run cbindgen to generate the C++ bindings for the Rust static library
add_custom_command(
  OUTPUT generated_includes/rust-plugin-1-bindings.h
  COMMAND
  cbindgen
            --config ${CMAKE_CURRENT_SOURCE_DIR}/../cbindgen.toml
            --crate rust-plugin-1
            --output generated_includes/rust-plugin-1-bindings.h
            --depfile ${CMAKE_CURRENT_BINARY_DIR}/generated_includes/rust-plugin-1-bindings.h.d
            ${CMAKE_CURRENT_SOURCE_DIR}/../
  # Make sure the bindings get regenerated correctly
  DEPFILE ${CMAKE_CURRENT_BINARY_DIR}/generated_includes/rust-plugin-1-bindings.h.d
  VERBATIM)
add_custom_target(rust_plugin_1_bindings DEPENDS generated_includes/rust-plugin-1-bindings.h)

add_library(${PROJECT_NAME}-impl STATIC "rust_plugin_1_clap_entry.cpp")
add_dependencies(${PROJECT_NAME}-impl rust_plugin_1_bindings)
target_include_directories(${PROJECT_NAME}-impl PUBLIC ${CMAKE_CURRENT_BINARY_DIR}/generated_includes)

target_link_libraries(${PROJECT_NAME}-impl PRIVATE
            # Link to the static library built from Rust. This staticlib includes the
            # CLAP entrypoints.
            rust_plugin_1
)
# Must link the dynamic libraries used by the rust static library
target_link_libraries(${PROJECT_NAME}-impl PUBLIC
            "-framework Foundation"
            "-framework CoreFoundation"
            "-framework AppKit"
            "-framework AudioToolbox"
            "-framework Quartz"
            "-framework CoreAudio"
            "-framework AudioUnit"
            "-framework CoreMIDI")

# Then we use make_clapfirst_plugin, provided by the wrappers, to create the
# project out of the impl and the cpp file which exports the entry
make_clapfirst_plugins(
        TARGET_NAME ${PROJECT_NAME}
        IMPL_TARGET ${PROJECT_NAME}-impl

        OUTPUT_NAME "${PROJECT_NAME}"

        ENTRY_SOURCE "rust_plugin_1_clap_entry.cpp"

        BUNDLE_IDENTIFER "org.free-audio.clap-rust-gain"
        BUNDLE_VERSION ${PROJECT_VERSION}

        COPY_AFTER_BUILD FALSE

        PLUGIN_FORMATS CLAP VST3 AUV2 WCLAP

        ASSET_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/${PROJECT_NAME}_assets

        # You can set up the AUv2 for a single plugin here, or you can
        # set it up with the auv2 extension in your clap
        AUV2_MANUFACTURER_NAME "Free Audio"
        AUV2_MANUFACTURER_CODE "FrAD"
        AUV2_SUBTYPE_CODE "Gain"
        AUV2_INSTRUMENT_TYPE "aufx"
)
