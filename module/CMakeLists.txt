cmake_minimum_required(VERSION 3.16)
project(AstraPrimitivesUiPlugin LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)
else()
    message(FATAL_ERROR "LogosModule.cmake not found. Set LOGOS_MODULE_BUILDER_ROOT.")
endif()

set(ASTRA_LOGOS_SDK_DIR "" CACHE PATH "Path to the Astra Logos GUI SDK")
if(NOT ASTRA_LOGOS_SDK_DIR)
    foreach(_candidate
            "${CMAKE_CURRENT_SOURCE_DIR}/../sdk"
            "${CMAKE_CURRENT_SOURCE_DIR}/sdk")
        if(EXISTS "${_candidate}/src/AstraLogosCliClient.h")
            set(ASTRA_LOGOS_SDK_DIR "${_candidate}")
            break()
        endif()
    endforeach()
endif()
if(NOT ASTRA_LOGOS_SDK_DIR)
    message(FATAL_ERROR "Astra Logos SDK not found. Set ASTRA_LOGOS_SDK_DIR.")
endif()

set(ASTRA_LOGOS_SDK_SOURCES
    ${ASTRA_LOGOS_SDK_DIR}/src/AstraLogosCliClient.h
    ${ASTRA_LOGOS_SDK_DIR}/src/AstraLogosCliClient.cpp
    ${ASTRA_LOGOS_SDK_DIR}/src/AstraLogosSchemas.h
    ${ASTRA_LOGOS_SDK_DIR}/src/AstraLogosSchemas.cpp
)

logos_module(
    NAME astra_primitives_ui
    REP_FILE src/astra_primitives_ui.rep
    SOURCES
        src/astra_primitives_backend.h
        src/astra_primitives_backend.cpp
        ${ASTRA_LOGOS_SDK_SOURCES}
    INCLUDE_DIRS
        src
        ${ASTRA_LOGOS_SDK_DIR}/src
)

option(ASTRA_LOGOS_BUILD_TESTS "Build Astra Logos GUI SDK unit tests" ON)
if(ASTRA_LOGOS_BUILD_TESTS)
    enable_testing()

    find_package(QT NAMES Qt6 Qt5 REQUIRED COMPONENTS Core Test)
    find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS Core Test)

    add_executable(astra_test_fake_cli
        tests/fake_astra_logos_cli.cpp
    )
    set_target_properties(astra_test_fake_cli PROPERTIES
        OUTPUT_NAME astra-logos-cli
    )
    target_link_libraries(astra_test_fake_cli PRIVATE
        Qt${QT_VERSION_MAJOR}::Core
    )

    add_executable(astra_logos_cli_client_test
        tests/astra_logos_cli_client_test.cpp
        ${ASTRA_LOGOS_SDK_SOURCES}
    )
    target_include_directories(astra_logos_cli_client_test PRIVATE
        ${ASTRA_LOGOS_SDK_DIR}/src
    )
    target_compile_definitions(astra_logos_cli_client_test PRIVATE
        "ASTRA_TEST_FAKE_CLI=\"$<TARGET_FILE:astra_test_fake_cli>\""
    )
    target_link_libraries(astra_logos_cli_client_test PRIVATE
        Qt${QT_VERSION_MAJOR}::Core
        Qt${QT_VERSION_MAJOR}::Test
    )
    add_dependencies(astra_logos_cli_client_test astra_test_fake_cli)
    add_test(NAME astra_logos_cli_client_test COMMAND astra_logos_cli_client_test)
endif()

if(TARGET astra_logos_cli_client_test)
 target_compile_definitions(astra_logos_cli_client_test PRIVATE ASTRA_SDK_TESTING)
endif()
