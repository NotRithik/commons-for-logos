# Separate binary directory prevents duplicate QtRO moc outputs.
  add_executable(commons_backend_state_test "${MOD}/tests/commons_backend_state_test.cpp"
    "${MOD}/src/commons_primitives_backend.h" "${MOD}/src/commons_primitives_backend.cpp" ${SDK_SOURCES})
  target_include_directories(commons_backend_state_test PRIVATE "${CMAKE_CURRENT_BINARY_DIR}" "${MOD}/src" "${SDK}" "${LOGOS_DEPS_DIR}/logos-view-module/cpp")
  target_link_libraries(commons_backend_state_test PRIVATE Qt6::Core Qt6::RemoteObjects Qt6::Test)
  target_compile_definitions(commons_backend_state_test PRIVATE COMMONS_SDK_TESTING "COMMONS_TEST_FAKE_CLI=\"$<TARGET_FILE:commons_test_fake_cli>\"")
  qt6_add_repc_sources(commons_backend_state_test "${MOD}/src/commons_primitives_ui.rep")
  add_dependencies(commons_backend_state_test commons_test_fake_cli)
  add_test(NAME commons_backend_state COMMAND commons_backend_state_test)
