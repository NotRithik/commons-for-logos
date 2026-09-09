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

## Basecamp theme and shared instance

The views import `Logos.Theme` and `Logos.Controls` from the installed Basecamp
host. Buttons, tabs, typography, background surfaces and status colors use the
same design tokens as Basecamp's own applications. No font files or substitute
theme are shipped. Private membership and shared approvals are tabs in the same
Commons module; the Commons Relay owner view is a separate module in the same host.

An operator can set `COMMONS_DEFAULT_CLI` and `COMMONS_DEFAULT_WALLET` in the
Basecamp launch environment. They are validated exactly like manually entered
settings and contain paths, not keys. With no defaults, the app starts unconfigured.


## Beginner walkthrough and safe viewing

Use **How to use Commons** in the installed app for the difference between private
membership and shared approvals. Commons is not the agent chat interface. The
membership implementation is an allowlist gate, not a token-airdrop payout.

Opening a saved workspace only reads its public testnet state. A workspace with
`.commons-readonly` is a key-free viewer: native controls and the CLI both reject
writes. It can contain the testnet marker, endpoint configuration and program IDs
without `storage.json`, private member keys or credentials. The label indicates
view-only mode; the last successful read includes its block number.

To register or approve, configure an actual member wallet and choose the private
credential supplied by its organizer. Review the action and wait for a confirmed
receipt. Private proving can take many minutes; a busy screen is not permission to
send the same transaction twice. Organizer setup remains an advanced flow: an
unused state account and genuine prepared membership commitment are required.

After the public testnet reset on 8 September, old catalog entries are labelled
as archives and a fresh live viewing workspace is separate. Historical receipts
are preserved; they are not used to draw a fake successful current state.


## Export a prepared demo credential for the GUI

The reproducible demo stores a protected Borsh bundle of all its synthetic members.
The GUI accepts one member credential at a time, not the whole bundle. After the
corresponding demo stage has stopped, export those already-created credentials:

```sh
cargo +1.94.0 fetch --locked --manifest-path credential-tool/Cargo.toml
cargo +1.94.0 run --locked --offline --manifest-path credential-tool/Cargo.toml -- /absolute/path/to/testnet-profile threshold
```

For membership use `distribution_a` or `distribution_b` instead of `threshold`.
Choose `ui-credentials/threshold/member-1.borsh` within that same profile using
**Select credential**. The exporter uses the published witness type. It never
opens a wallet, generates a key, starts a proof, sends a transaction or prints
private content. Files have owner-only read/write permissions; existing mismatched
files are preserved, not overwritten.

These test profiles contain the demo organizer's synthetic members. They are not
a production identity-distribution mechanism. Do not upload the credentials or
wallet profiles or expose their contents in a recording. The wallet must hold the
matching member key; importing a credential alone is not sufficient.


## Workspace menu and action availability

The workspace picker is rendered with the shared Logos palette inside the
Basecamp scene. Its popup is capped, scrollable, keyboard-accessible, and closes
with Escape without selecting or submitting anything. It does not use a native
macOS menu or change the application's global control style.

Refresh reads the selected account before actions become available. Completed
proposals cannot be executed again. A new proposal needs an exact signed
64-bit value and a selected private credential; a pending proposal needs the
required distinct approvals before Execute becomes available. Full membership
lists explain that no registrations remain. These are UI safeguards; the native
client and on-chain program independently revalidate every request.

See **Export a prepared demo credential for the GUI** above for the protected
credential conversion. The workspace menu itself never imports a credential or
submits a transaction.
