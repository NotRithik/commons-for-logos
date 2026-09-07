#!/usr/bin/env node

// TEST HARNESS wrapper. The builder auto-runs module/tests/ui-tests.mjs when
// Nix is available; this repository-level entry point is for manual UI runs.
await import("../module/tests/ui-tests.mjs");
