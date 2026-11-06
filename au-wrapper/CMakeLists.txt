# Builds "Hexa FM.component" (Audio Unit v2) by wrapping the Hexa FM CLAP with free-audio/clap-wrapper.
# The CLAP is embedded inside the component bundle, so the AU works on its own.
#
#   cmake -S au-wrapper -B target/au-build -DHEXA_CLAP="target/bundled/Hexa FM.clap"
#   cmake --build target/au-build --config Release
cmake_minimum_required(VERSION 3.21)
set(CMAKE_OSX_DEPLOYMENT_TARGET "11.0" CACHE STRING "")
project(HexaFM_AU VERSION 0.1.0 LANGUAGES C CXX OBJC OBJCXX)
set(CMAKE_CXX_STANDARD 17)

if (NOT HEXA_CLAP)
    message(FATAL_ERROR "Pass -DHEXA_CLAP=/path/to/Hexa FM.clap")
endif()

set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES TRUE CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_BUILD_AUV2 TRUE CACHE BOOL "" FORCE)
include(FetchContent)
FetchContent_Declare(clap_wrapper
    GIT_REPOSITORY https://github.com/free-audio/clap-wrapper.git
    GIT_TAG 806bba0a5f792e3966b99c1561ab9952d3316d7c)
FetchContent_MakeAvailable(clap_wrapper)

add_library(HexaFM_auv2 MODULE)
target_add_auv2_wrapper(
    TARGET HexaFM_auv2
    OUTPUT_NAME "Hexa FM"
    BUNDLE_IDENTIFIER "app.hexafm.synth.auv2"
    BUNDLE_VERSION "0.1.0"
    INSTRUMENT_TYPE "aumu"
    MANUFACTURER_NAME "Hexa FM"
    MANUFACTURER_CODE "HxFm"
    SUBTYPE_CODE "Hx6o"
    MACOS_EMBEDDED_CLAP_LOCATION "${HEXA_CLAP}"
)
