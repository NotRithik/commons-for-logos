# Separate binary directory prevents duplicate QtRO moc outputs.
  add_executable(astra_backend_state_test "${MOD}/tests/astra_backend_state_test.cpp"
    "${MOD}/src/astra_primitives_backend.h" "${MOD}/src/astra_primitives_backend.cpp" ${SDK_SOURCES})
  target_include_directories(astra_backend_state_test PRIVATE "${CMAKE_CURRENT_BINARY_DIR}" "${MOD}/src" "${SDK}" "${LOGOS_DEPS_DIR}/logos-view-module/cpp")
  target_link_libraries(astra_backend_state_test PRIVATE Qt6::Core Qt6::RemoteObjects Qt6::Test)
  target_compile_definitions(astra_backend_state_test PRIVATE ASTRA_SDK_TESTING "ASTRA_TEST_FAKE_CLI=\"$<TARGET_FILE:astra_test_fake_cli>\"")
  qt6_add_repc_sources(astra_backend_state_test "${MOD}/src/astra_primitives_ui.rep")
  add_dependencies(astra_backend_state_test astra_test_fake_cli)
  add_test(NAME astra_backend_state COMMAND astra_backend_state_test)
