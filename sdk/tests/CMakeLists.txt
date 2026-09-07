cmake_minimum_required(VERSION 3.20)
project(AstraSdkVerification LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
find_package(Qt6 6.6 REQUIRED COMPONENTS Core Test)
get_filename_component(REPO "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(SDK "${REPO}/sdk/src")
add_executable(astra_test_fake_cli "${REPO}/module/tests/fake_astra_logos_cli.cpp")
set_target_properties(astra_test_fake_cli PROPERTIES OUTPUT_NAME astra-logos-cli)
target_link_libraries(astra_test_fake_cli PRIVATE Qt6::Core)
add_executable(astra_logos_cli_client_test
 "${REPO}/module/tests/astra_logos_cli_client_test.cpp"
 "${SDK}/AstraLogosCliClient.cpp" "${SDK}/AstraLogosCliClient.h"
 "${SDK}/AstraLogosSchemas.cpp" "${SDK}/AstraLogosSchemas.h")
target_include_directories(astra_logos_cli_client_test PRIVATE "${SDK}")
target_compile_definitions(astra_logos_cli_client_test PRIVATE ASTRA_SDK_TESTING
 "ASTRA_TEST_FAKE_CLI=\"$<TARGET_FILE:astra_test_fake_cli>\"")
target_link_libraries(astra_logos_cli_client_test PRIVATE Qt6::Core Qt6::Test)
add_dependencies(astra_logos_cli_client_test astra_test_fake_cli)
enable_testing()
add_test(NAME sdk-process-boundary COMMAND astra_logos_cli_client_test)
set_tests_properties(sdk-process-boundary PROPERTIES TIMEOUT 60)
