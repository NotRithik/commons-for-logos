# Build the native Basecamp module

## Inputs

This builds a genuine Qt Remote Objects Basecamp module, not a browser simulation. Tested locally on macOS arm64 with Qt 6.9.2 and Basecamp 0.2.3. Linux checks are run separately in CI; consult their actual results.

Prerequisites: Python 3, CMake 3.20+, a C++17 compiler, git, curl, and libarchive's `bsdtar`. On Linux, Qt also needs ordinary OpenGL/fontconfig/XKB system libraries. `native/qt-archives.json` pins exact official Qt archives by HTTPS URL, size and SHA256; `scripts/fetch-native-deps.py` pins the three official Logos source revisions.

## Clean checkout

```sh
python3 scripts/fetch-qt.py "$PWD/.build-deps/qt"
python3 scripts/fetch-native-deps.py "$PWD/.build-deps/logos"
export QT_PREFIX="$PWD/.build-deps/qt/prefix"
export LOGOS_DEPS_DIR="$PWD/.build-deps/logos"
export ASTRA_BUILD_JOBS=2
/bin/sh scripts/build-native.sh
```

Downloads and builds are separate. Inspect the small scripts before using them; no downloaded installer or pipe-to-shell command is used. The native build script itself does not need network access. The Qt archive fetcher verifies all inputs before extraction and refuses an unknown nonempty SDK destination.

Outputs default to `out/native` and `out/module`. Override `ASTRA_BUILD_DIR` and `ASTRA_INSTALL_DIR` for another directory. The test-only executable is built in a separate `test-only` directory and is **not installed**.

## Use with Basecamp

The installed output contains the plugin, replica factory, metadata and QML. Basecamp supplies its compatible Qt runtime. The Rust `astra-logos-cli` is a separate executable built from `cli/Cargo.toml`; the GUI never falls back to the fake test executable.

Configure the GUI with the actual production CLI path and a dedicated, marked testnet wallet directory. This requires real program artifacts and the correct `programs.json`, not arbitrary addresses pasted into a mock backend. See `sdk/CLI-CONTRACT.md` and `docs/GUI.md`.

Do not distribute the full Qt development SDK or its font assets with this project. Do not remove operating-system verification globally to open an application. Locally built macOS plugins may be ad-hoc signed as developer artifacts; that is not a notarization claim.

## macOS window registration and AutoFill

The published Basecamp 0.2.3 macOS bundle uses a shell launcher named
`LogosBasecamp`, which executes `LogosBasecamp.bin`. When launched directly under
our restricted test runner, macOS reported an empty running-app bundle ID and
computer-use tools could not attach by bundle identifier. A separate local
developer copy with `CFBundleExecutable=LogosBasecamp.bin` and a distinct bundle
ID fixed attachment. The original download was kept unchanged. The developer
copy was ad-hoc signed and is not a notarized release.

Use a visible Cocoa window for interactive tests, not `QT_QPA_PLATFORM=offscreen`.
Tests that create a window should say whether they use the actual app or a unit
fixture. A screenshot of a fixture is not evidence of a live network action.

The optional process-local `ASTRA_DISABLE_NATIVE_AUTOFILL=1` workaround applies
only to macOS 26.0 and 26.1. Its preference was removed in 26.2; the initializer
and regression test are version-gated accordingly. It does not modify persistent
user defaults. See Chromium's upstream notes in
`https://chromium.googlesource.com/chromium/src/+/main/content/app/mac_init.mm`.

On the newer OS used for this run, denying the AppKit
`com.apple.SafariPlatformSupport.Helper` connection produced a retry loop. The
GUI-only sandbox allowed that specific OS helper, after which idle CPU dropped
to near zero. It still did not grant access to Safari's files, user wallet files,
or general home-directory contents. Build and proof profiles were unchanged.

## Verification boundary

The Qt process-boundary suite intentionally drives a labeled test fixture to exercise input validation, cancellation, timeouts, malformed output and environment isolation. It does not claim to prove a LEZ transaction. Actual GUI and chain evidence is collected independently.
