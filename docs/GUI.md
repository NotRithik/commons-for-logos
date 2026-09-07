# Commons Basecamp app

Commons runs as a native QML module in Logos Basecamp. It uses Qt Remote Objects
to reach its backend and a typed client to launch `commons-logos-cli` with fixed
arguments. It does not render a browser mock-up or display synthetic transactions
as live data.

## Install

Open Basecamp's Package Manager and install the macOS ARM64 `.lgx` from this
repository's release. The package includes the UI plugin, replica factory, QML,
icon and licenses. It does not include a wallet, witness, fake CLI, Qt SDK or fonts.

The CLI is a separate executable. Build it using `scripts/prepare-local.sh`, or
use a verified CLI download provided with the release. Configure its absolute
path and a dedicated testnet wallet in **Connection settings**. The wallet must
have the expected testnet marker, configuration and `programs.json`.

The standard app uses the official LEZ endpoint. A local sequencer is needed only
for the separate reproducible development/CI test. Proof generation happens on
the user's machine through RISC Zero; no hosted model or prover API is used.

## Workflows

**Membership:** create a distribution from an eligibility root, load the public
state account, select an eligible member's witness file, and claim privately.
**Shared approvals:** create a threshold group, propose a value, gather distinct
private approvals, then execute the approved change. Read the state again to
confirm the result.

A testnet submission can take several minutes while its local proof is generated.
Do not treat a pending operation as confirmed. The app displays success only
after its CLI returns a confirmed result. Witness files and test wallets must not
be shared publicly. See `PRIVACY.md` and `sdk/CLI-CONTRACT.md`.

## Local evidence capture

Expand **Technical details** for **Save view** and **Record view**. They capture
only this module, not the desktop or another application. Connection settings
are collapsed before capture. Files are stored inside the configured test
wallet's `evidence` directory; no upload occurs.

Recording saves a frame every 1.5 seconds and stops after 2,400 saved frames
(one hour). **Stop recording** ends it immediately; changing configuration also
stops it. The counter advances only when an image was successfully written.
A short recording test saved 24 frames and verified that Stop changed the UI to
its stopped state. This is recording-tool validation, not the full prize demo.

## Verification performed

The visible macOS app was driven through its real controls. Checks covered
configuration, wrong-format account rejection, recovery to a valid live read,
allowlist/threshold navigation, module-only screenshots, and recording/start/stop.
The final release's allowlist and group were read from the public testnet.

The LGX file passed the official library's integrity verification. Modifying a
QML file without changing its manifest hash was rejected. The actual package
manager library installed it into a fresh isolated plugin directory, and every
installed payload byte matched the package. The install-check script does not
claim that it exercised the Package Manager GUI.

Native process-boundary unit tests use an explicitly labeled fake CLI for error
cases; those tests are separate from the real CLI and testnet reads. No fake CLI
is included in the installed module.

## Build and reproduce

See `NATIVE-BUILD.md` for the pinned native build inputs. Package a built output
using the official LGX library from Basecamp:

```sh
python3 scripts/package-lgx.py --lgx-lib /path/to/liblgx.dylib \
  --native-dir out/module --variant darwin-arm64 \
  --output out/releases/commons-macos-arm64.lgx
```

`package-lgx.py` includes only named module files. `check-native-package.py`
verifies integrity, performs the deliberately corrupted-copy rejection test,
and installs into a new test directory. It never replaces an existing install.
