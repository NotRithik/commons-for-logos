cmake_minimum_required(VERSION 3.16)
project(CommonsPrimitivesUiPlugin LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)
else()
    message(FATAL_ERROR "LogosModule.cmake not found. Set LOGOS_MODULE_BUILDER_ROOT.")
endif()

set(COMMONS_LOGOS_SDK_DIR "" CACHE PATH "Path to the Commons Logos GUI SDK")
if(NOT COMMONS_LOGOS_SDK_DIR)
    foreach(_candidate
            "${CMAKE_CURRENT_SOURCE_DIR}/../sdk"
            "${CMAKE_CURRENT_SOURCE_DIR}/sdk")
        if(EXISTS "${_candidate}/src/CommonsLogosCliClient.h")
            set(COMMONS_LOGOS_SDK_DIR "${_candidate}")
            break()
        endif()
    endforeach()
endif()
if(NOT COMMONS_LOGOS_SDK_DIR)
    message(FATAL_ERROR "Commons Logos SDK not found. Set COMMONS_LOGOS_SDK_DIR.")
endif()

set(COMMONS_LOGOS_SDK_SOURCES
    ${COMMONS_LOGOS_SDK_DIR}/src/CommonsLogosCliClient.h
    ${COMMONS_LOGOS_SDK_DIR}/src/CommonsLogosCliClient.cpp
    ${COMMONS_LOGOS_SDK_DIR}/src/CommonsLogosSchemas.h
    ${COMMONS_LOGOS_SDK_DIR}/src/CommonsLogosSchemas.cpp
)

logos_module(
    NAME commons_primitives_ui
    REP_FILE src/commons_primitives_ui.rep
    SOURCES
        src/commons_primitives_backend.h
        src/commons_primitives_backend.cpp
        ${COMMONS_LOGOS_SDK_SOURCES}
    INCLUDE_DIRS
        src
        ${COMMONS_LOGOS_SDK_DIR}/src
)

option(COMMONS_LOGOS_BUILD_TESTS "Build Commons Logos GUI SDK unit tests" ON)
if(COMMONS_LOGOS_BUILD_TESTS)
    enable_testing()

    find_package(QT NAMES Qt6 Qt5 REQUIRED COMPONENTS Core Test)
    find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS Core Test)

    add_executable(commons_test_fake_cli
        tests/fake_commons_logos_cli.cpp
    )
    set_target_properties(commons_test_fake_cli PROPERTIES
        OUTPUT_NAME commons-logos-cli
    )
    target_link_libraries(commons_test_fake_cli PRIVATE
        Qt${QT_VERSION_MAJOR}::Core
    )

    add_executable(commons_logos_cli_client_test
        tests/commons_logos_cli_client_test.cpp
        ${COMMONS_LOGOS_SDK_SOURCES}
    )
    target_include_directories(commons_logos_cli_client_test PRIVATE
        ${COMMONS_LOGOS_SDK_DIR}/src
    )
    target_compile_definitions(commons_logos_cli_client_test PRIVATE
        "COMMONS_TEST_FAKE_CLI=\"$<TARGET_FILE:commons_test_fake_cli>\""
    )
    target_link_libraries(commons_logos_cli_client_test PRIVATE
        Qt${QT_VERSION_MAJOR}::Core
        Qt${QT_VERSION_MAJOR}::Test
    )
    add_dependencies(commons_logos_cli_client_test commons_test_fake_cli)
    add_test(NAME commons_logos_cli_client_test COMMAND commons_logos_cli_client_test)
endif()

if(TARGET commons_logos_cli_client_test)
 target_compile_definitions(commons_logos_cli_client_test PRIVATE COMMONS_SDK_TESTING)
endif()
