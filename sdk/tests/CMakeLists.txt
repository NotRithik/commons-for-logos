cmake_minimum_required(VERSION 3.20)
project(CommonsSdkVerification LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
find_package(Qt6 6.6 REQUIRED COMPONENTS Core Test)
get_filename_component(REPO "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(SDK "${REPO}/sdk/src")
add_executable(commons_test_fake_cli "${REPO}/module/tests/fake_commons_logos_cli.cpp")
set_target_properties(commons_test_fake_cli PROPERTIES OUTPUT_NAME commons-logos-cli)
target_link_libraries(commons_test_fake_cli PRIVATE Qt6::Core)
add_executable(commons_logos_cli_client_test
 "${REPO}/module/tests/commons_logos_cli_client_test.cpp"
 "${SDK}/CommonsLogosCliClient.cpp" "${SDK}/CommonsLogosCliClient.h"
 "${SDK}/CommonsLogosSchemas.cpp" "${SDK}/CommonsLogosSchemas.h")
target_include_directories(commons_logos_cli_client_test PRIVATE "${SDK}")
target_compile_definitions(commons_logos_cli_client_test PRIVATE COMMONS_SDK_TESTING
 "COMMONS_TEST_FAKE_CLI=\"$<TARGET_FILE:commons_test_fake_cli>\"")
target_link_libraries(commons_logos_cli_client_test PRIVATE Qt6::Core Qt6::Test)
add_dependencies(commons_logos_cli_client_test commons_test_fake_cli)
enable_testing()
add_test(NAME sdk-process-boundary COMMAND commons_logos_cli_client_test)
set_tests_properties(sdk-process-boundary PROPERTIES TIMEOUT 60)
