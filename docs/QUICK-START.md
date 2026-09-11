# Use Commons without learning its blockchain internals

Commons has two jobs: let an eligible person register once without publishing their member address, and let a group change a shared setting only after enough private approvals. It runs inside Logos Basecamp and uses the Logos testnet. Do not put real money in its demo wallets.

## The three things you choose

**Your identity** is your local member wallet. It contains the private key used to prove that you are an eligible member. Give it a recognizable name. Creating an identity does not publish a transaction, request faucet funds, or vote. The wallet files are stored on your computer with owner-only permissions; they are not encrypted custody storage.

**Your workspace** is a named membership list or decision policy. For example, “Community membership” or “Community grant limit.” Choose its name in **My workspaces**. You do not need to type an account address or calculate a membership root.

**Your action** is separate from opening a workspace. Opening and refreshing read public testnet state. Registering, proposing, approving, publishing a draft, and applying an approved change require their own button and review.

A policy's blockchain reference is the public address of that policy, like the address of a shared document. It is not your private key or a password. Knowing the reference does not grant membership or voting rights. The app puts it under **Blockchain reference (advanced)** for people who need to cross-check a deployment.

## Before the first use

Install the Commons `.lgx` package in Basecamp's Package Manager and unpack the matching `commons-logos-cli` companion. Keep the `artifacts` directory next to that executable: its two program files must match the published manifest. Do not mix an older client with a newer module.

For a new key-free reader, run this from the source checkout with a new destination folder:

```sh
python3 scripts/create-reader.py "$HOME/Commons Testnet Reader"
```

The helper creates only public testnet configuration and a read-only marker. It never replaces an existing wallet, creates keys, contacts a faucet, or sends a transaction. No blockchain address needs to be entered to create your own identity afterwards.

In **Connection settings**, choose the Commons executable and a prepared testnet workspace. A key-free viewing workspace is enough to start the local identity setup. The native-build and GUI guides describe developer setup and the reproducible local stack. The `.lgx` does not contain someone else's wallet, private credentials, a fake client, or a Qt development installation.

Real private actions also require the pinned local RISC Zero prover and proof dependencies. A reader can inspect public state without those private wallet files. A completed build is not a completed blockchain action.

## Create your identity

Open **My workspaces**, choose **Load identities** to recover an existing local library, or **New identity** for a new member. Do not create another identity just because a network operation is slow. After a restart, load the existing identities first.

Use **Share enrollment** when an organizer asks to include you. This is a public membership document containing a member address, label, and membership salt, not your signing key. Share it privately with the organizer. Never send the wallet folder, `storage.json`, a mnemonic, or a private member credential.

For a demonstration on one computer, separate named identities have separate wallets. For a real group, each participant should keep their own identity on their own device. One person operating three demo identities does not demonstrate three independent humans.

## Organize a membership list

Choose **Create membership list**, give the list a name, and add the intended members' enrollment documents. **Add my selected identity** includes you when appropriate. The app prevents adding the same member twice. Review the names before preparing the list.

**Prepare membership list** saves a local draft and individual invitations. It does not publish the list or register anyone. The organizer then selects **Review first publication** and confirms the separate testnet action. An already-created list cannot be initialized again at the same address.

Give each person their own invitation from **Member invitations**. A participant selects their own identity and uses **Join with invitation**. An invitation for someone else is rejected. The app checks its membership path and derives the private credential locally. The organizer does not need the participant's signing key.

When you are included in a workspace you created, **Enable my membership** performs that same local invitation step for your own identity. Being the organizer is not, by itself, proof that you are an eligible member.

## Register privately

Open the saved membership list. Check the current number of registrations, then choose **Register privately** and review the action. Wait for the confirmed result. Each eligible member can register only once in that list; a rejected duplicate is not another successful registration.

Different lists have different eligibility commitments and registration records. A credential for one list cannot be used as membership in another. This implementation is a membership gate. It does not send an airdrop or transfer tokens.

## Create a shared decision policy

Choose **Create a policy**, or start with **Grant limit**, **Storage quota**, or **Custom integer setting**. Set the policy name, the name of the setting, its starting value, the member enrollments, and how many distinct approvals are required. For example, two of three members may approve changing a limit from 100 to 125.

The templates create separate instances of the same threshold program, not newly compiled smart-contract code. The chain enforces the membership commitment, approval threshold, and whole-number value. Names, currency units, and application-specific meanings are local labels. A grant-limit policy does not send a grant; a storage-quota policy does not provision storage. Another application can use the approved value as its input.

Prepare the draft, review the first publication, distribute individual invitations, and enable your own membership when included. An approval policy's membership set and threshold are fixed at creation in this version.

## Propose, approve, and apply a change

Open the named decision and refresh its state. The screen shows its current value and whether a proposal is waiting. Enter a whole number and choose **Propose**. Review the exact value and current state before confirming. Creating a proposal does not automatically cast your approval or change the current value.

Each participating member opens the same policy under their own identity, checks the proposed value, and chooses **Approve privately**. The screen shows how many distinct approvals are present, not the members' voting identities. The program rejects a second approval from the same member for the same proposal.

Before the required number of approvals is reached, the change cannot be applied. Once enough approvals exist, **Apply change** applies the approved value. This final relay is public and does not require the creator's signing key. Read the resulting state and check that the intended proposal is marked executed and the current value matches it.

The review is bound to the public state shown. A state change while you are reviewing requires a fresh read and review. This is a client check, not a new consensus rule added to the deployed legacy guest. The client also checks the exact result after execution instead of accepting an old transaction hash as success.

## When something takes time or fails

Private proofs are generated locally and can take many minutes. Keep the application running. A busy screen, elapsed timer, or accepted request does not mean the chain has confirmed the action. Do not submit another copy because it seems slow.

If the app reports a missing invitation, enable or import membership for the selected identity. If it reports a stale review, refresh and review the new state. If it reports a wrong identity or wrong-list credential, select the intended identity or invitation; do not copy someone else's private key.

If a submitted transaction has an uncertain outcome, inspect and reconcile its recorded transaction before another write. Do not delete pending checkpoint files to force a retry. An operation that failed before any submission and a lost reply after submission are different cases.

A testnet reset can remove earlier deployments or make old wallet history incompatible. Historical receipts remain evidence of what happened then, not proof that a list still exists now. Keep an affected wallet as an archive instead of silently resetting it or replaying its old proofs.

## What stays private, and what does not

Member secret keys and private credentials stay on their devices. Public application state does not contain the member address list or reveal which member approved. Group settings, proposal values, approval and registration counts, transaction timing, and the public state reference are observable. The organizer sees enrollments supplied to them; this is not anonymity from an organizer who already knows those members. Network traffic and off-chain coordination can leak information beyond the application state.

See [PRIVACY.md](PRIVACY.md), [PROTOCOL.md](PROTOCOL.md), and [ERRORS.md](ERRORS.md) for the precise assumptions and limitations. Keep enrollment/invitation documents and private settings out of a public recording. Show the named workspace, reviewed action, proof status, and confirmed public outcome instead.
