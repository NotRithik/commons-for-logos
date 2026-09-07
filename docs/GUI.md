# Commons Primitives Basecamp GUI

This repository now includes a loadable Logos Basecamp `ui_qml` module for the
LP-0002 threshold parameter workflow and the LP-0003 allowlist registration
workflow.

## Files

- `module/` contains the Basecamp QML module:
  - `metadata.json`
  - `flake.nix`
  - `CMakeLists.txt`
  - `src/commons_primitives_ui.rep`
  - `src/commons_primitives_backend.{h,cpp}`
  - `src/qml/Main.qml`
- `sdk/` contains the reusable Qt SDK layer:
  - `src/CommonsLogosCliClient.{h,cpp}`
  - `src/CommonsLogosSchemas.{h,cpp}`
  - `schema/commons-logos-cli.schema.json`
  - `CLI-CONTRACT.md`
- `module/tests/` contains Qt unit tests and the builder-discovered UI test.
- `ui-tests/` contains a repository-level test harness wrapper.

## Runtime Policy

The GUI never constructs or signs live private transactions itself. It only
delegates to an explicitly configured absolute `commons-logos-cli` executable.
The backend:

- requires the executable to be named `commons-logos-cli` or `commons-logos-cli.exe`;
- starts it with `QProcess::setProgram()` and fixed argument lists;
- sends operation inputs as JSON stdin;
- forces `RISC0_DEV_MODE=0` and `COMMONS_LOGOS_NETWORK=testnet`;
- requires a wallet directory that is visibly testnet-only by path or marker;
- treats malformed CLI output as failure and does not display raw stdout/stderr;
- redacts sensitive witness, secret, key, seed, mnemonic, password, and selected
  path fields from JSON displayed in the UI.

No mock transaction data is rendered as live state. The summary panels remain in
their default "No live state loaded" state until the configured CLI returns a
successful JSON response.

## CLI Contract

The CLI request and response contract is documented in `sdk/CLI-CONTRACT.md`.
The SDK exposes the same schema through `CommonsLogos::contractSchemaJson()`.

Fixed operations:

- `allowlist.create_distribution`
- `allowlist.claim`
- `allowlist.inspect_state`
- `threshold.create_group`
- `threshold.propose`
- `threshold.approve`
- `threshold.execute`
- `threshold.inspect_state`

Witness file contents are never passed on argv. Witness paths are carried in the
stdin JSON request.

## Build

The module flake pins `logos-module-builder` to the checked-out builder commit:

```text
1c2532b2de614c0cd2fc68544fc7b1efed944363
```

When Nix is available:

```bash
cd module
nix build .#default
nix build .#ui-dev
nix build .#integration-test -L
```

For local testing in this repository, run those commands through the repository
sandbox wrapper:

```bash
/bin/sh ../../tools/run-isolated.sh local nix build ./module#default
/bin/sh ../../tools/run-isolated.sh local nix build ./module#integration-test -L
```

The wrapper path is not executable in this checkout, so direct execution may
fail with `permission denied`; invoking it through `/bin/sh` still uses the same
reviewed script.

## Tests

Qt unit tests are built from `module/CMakeLists.txt` when
`COMMONS_LOGOS_BUILD_TESTS=ON`. They cover:

- CLI executable and testnet wallet validation;
- fixed argv for allowlisted operations;
- JSON stdin transport for witness paths;
- `RISC0_DEV_MODE=0` enforcement;
- invalid JSON failure handling;
- redaction of wallet and witness paths in failures;
- threshold bound validation before spawning a process.

The UI test at `module/tests/ui-tests.mjs` covers real QML controls by
objectName through the Logos Qt inspector. It verifies:

- the module loads in explicit `Not configured` state;
- transaction buttons are disabled before configuration;
- invalid configuration is handled by the backend;
- the threshold tab exposes proposal, approve, execute, and inspect controls.

The repository wrapper `ui-tests/commons_primitives_ui.test.mjs` imports the same
suite for manual runs.

## Current Local Blockers

This managed environment cannot apply the sandbox profile:

```text
sandbox-exec: sandbox_apply: Operation not permitted
```

Because local compilation and tests must go through `../../tools/run-isolated.sh`,
the Rust tests, Nix build, Qt unit tests, and QML integration tests were not run
locally from this environment. `git diff --check` passed.
