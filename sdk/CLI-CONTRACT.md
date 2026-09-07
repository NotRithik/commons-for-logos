# Astra Logos CLI Contract

This SDK does not construct private Logos transactions itself. The Basecamp GUI
delegates every live LP-0002 and LP-0003 transaction action to an explicitly
configured absolute `astra-logos-cli` executable.

## Invocation

The GUI starts the CLI with `QProcess::setProgram()` and a fixed argument
allowlist. It never invokes a shell.

| Operation | CLI arguments |
| --- | --- |
| `allowlist.create_distribution` | `primitives allowlist-create --json-stdin` |
| `allowlist.claim` | `primitives allowlist-claim --json-stdin` |
| `allowlist.inspect_state` | `primitives allowlist-inspect --json-stdin` |
| `threshold.create_group` | `primitives threshold-create --json-stdin` |
| `threshold.propose` | `primitives threshold-propose --json-stdin` |
| `threshold.approve` | `primitives threshold-approve --json-stdin` |
| `threshold.execute` | `primitives threshold-execute --json-stdin` |
| `threshold.inspect_state` | `primitives threshold-inspect --json-stdin` |

For every invocation the environment must contain `RISC0_DEV_MODE=0` and
`ASTRA_LOGOS_NETWORK=testnet`. The CLI must reject any non-testnet wallet or
configuration even if the GUI already performed local path checks.

## Request

All operation inputs are sent as one compact JSON object on stdin:

```json
{
  "schema_version": 1,
  "network": "testnet",
  "wallet_dir": "/absolute/path/to/testnet-wallet",
  "operation": "allowlist.claim",
  "arguments": {
    "state_account": "64-byte-hex-account-id",
    "witness_file": "/absolute/path/to/private-witness.json"
  }
}
```

Witness contents must never be passed on the command line, echoed in logs, or
returned in CLI stdout. The GUI only passes a witness file path inside stdin.

## Arguments

`allowlist.create_distribution`

```json
{ "root": "64-byte-hex-root", "member_count": 10 }
```

`allowlist.claim`

```json
{ "state_account": "64-byte-hex-account-id", "witness_file": "/absolute/path" }
```

`allowlist.inspect_state`

```json
{ "state_account": "64-byte-hex-account-id" }
```

`threshold.create_group`

```json
{
  "root": "64-byte-hex-root",
  "member_count": 10,
  "threshold": 3,
  "initial_value": "7"
}
```

`threshold.propose`

```json
{
  "state_account": "64-byte-hex-account-id",
  "witness_file": "/absolute/path",
  "next_value": "42"
}
```

`threshold.approve`

```json
{ "state_account": "64-byte-hex-account-id", "witness_file": "/absolute/path" }
```

`threshold.execute` and `threshold.inspect_state`

```json
{ "state_account": "64-byte-hex-account-id" }
```

`initial_value` and `next_value` are decimal strings so the UI never loses i64
precision through QML or JSON number conversion.

## Response

Success responses must be JSON objects with `success: true`:

```json
{
  "success": true,
  "tx_hash": "optional-transaction-hash",
  "state_account": "64-byte-hex-account-id",
  "state": {
    "member_count": 10,
    "claims_count": 1,
    "threshold": 3,
    "value": "7",
    "proposal": {
      "sequence": 1,
      "next_value": "42",
      "approvals_count": 2,
      "executed": false
    }
  }
}
```

Failure responses must be JSON objects with `success: false` and an `error`
string or `{ "message": "..." }` object. The GUI displays sanitized errors and
does not display raw stdout or stderr if parsing fails.
