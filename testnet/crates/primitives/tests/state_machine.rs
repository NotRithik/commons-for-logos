use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::*;
use lee_core::{
    NullifierPublicKey,
    account::{Account, AccountId, AccountWithMetadata, Nonce},
    encryption::ViewingPublicKey,
    program::{AccountPostState, Claim, DEFAULT_PROGRAM_ID, ProgramId, validate_execution},
};

struct Fixture {
    program: ProgramId,
    state: AccountWithMetadata,
    members: Vec<AccountWithMetadata>,
    witnesses: Vec<MemberWitness>,
    root: Hash32,
}

impl Fixture {
    fn new(size: usize) -> Self {
        let program = [41; 8];
        let state = AccountWithMetadata {
            account: Account::default(),
            is_authorized: true,
            account_id: AccountId::new([42; 32]),
        };
        let scope = context(program, state.account_id);
        let mut members = Vec::new();
        let mut witnesses = Vec::new();
        for index in 0..size {
            let secret = hash_parts(
                b"fixture/not-for-real-money",
                &[&(index as u64).to_le_bytes()],
            );
            let viewing = ViewingPublicKey::from_seed(&secret, &[7; 32]);
            let viewing_public_key = ViewingPublicKeyBytes::from_viewing_public_key(&viewing);
            let id = regular_private_account_id(
                &NullifierPublicKey::from(&secret),
                &viewing_public_key,
                index as u128,
            )
            .unwrap();
            let account = Account {
                // Existing private accounts need not be zero-nonce or owned by this program.
                nonce: (17 + index as u128).into(),
                program_owner: [99; 8],
                data: vec![index as u8, 99].try_into().unwrap(),
                ..Account::default()
            };
            members.push(AccountWithMetadata {
                account,
                is_authorized: true,
                account_id: id,
            });
            witnesses.push(MemberWitness {
                leaf: MemberLeaf {
                    account_id: id,
                    salt: hash_parts(b"fixture/salt", &[&secret]),
                    entitlement: 1,
                },
                nullifier_secret_key: secret,
                viewing_public_key,
                identifier: index as u128,
                leaf_index: index as u32,
                siblings: Vec::new(),
            });
        }
        let leaves: Vec<_> = witnesses
            .iter()
            .map(|w| w.leaf.commitment(&scope))
            .collect();
        let root = merkle_proof(&leaves, 0).unwrap().0;
        for (index, witness) in witnesses.iter_mut().enumerate() {
            witness.siblings = merkle_proof(&leaves, index).unwrap().1;
        }
        Self {
            program,
            state,
            members,
            witnesses,
            root,
        }
    }

    fn apply(&mut self, posts: Vec<AccountPostState>, state_signed: bool) {
        // Unit-test application of program outputs only. This is explicitly NOT
        // a sequencer, a proof verifier, or evidence of a testnet deployment.
        let state_post = &posts[0];
        self.state.account = state_post.account().clone();
        if state_post.required_claim().is_some() {
            self.state.account.program_owner = self.program;
        }
        if state_signed {
            self.state.account.nonce.public_account_nonce_increment();
        }
    }

    fn execute_distribution_checked(
        &self,
        accounts: &[AccountWithMetadata],
        instruction: DistributionInstruction,
    ) -> Result<Vec<AccountPostState>, Error> {
        let posts = execute_distribution(self.program, accounts, instruction)?;
        validate_execution(accounts, &posts, self.program).unwrap();
        Ok(posts)
    }

    fn execute_group_checked(
        &self,
        accounts: &[AccountWithMetadata],
        instruction: GroupInstruction,
    ) -> Result<Vec<AccountPostState>, Error> {
        let posts = execute_group(self.program, accounts, instruction)?;
        validate_execution(accounts, &posts, self.program).unwrap();
        Ok(posts)
    }

    fn distribution(&mut self) {
        let out = self
            .execute_distribution_checked(
                &[self.state.clone()],
                DistributionInstruction::Create {
                    root: self.root,
                    member_count: self.members.len() as u32,
                },
            )
            .unwrap();
        self.apply(out, true);
    }

    fn group(&mut self, threshold: u32) {
        let out = self
            .execute_group_checked(
                &[self.state.clone()],
                GroupInstruction::Create {
                    root: self.root,
                    member_count: self.members.len() as u32,
                    threshold,
                    initial_value: 7,
                },
            )
            .unwrap();
        self.apply(out, true);
    }

    fn claim_with(
        &self,
        index: usize,
        witness: MemberWitness,
    ) -> Result<Vec<AccountPostState>, Error> {
        self.execute_distribution_checked(
            &[self.state.clone(), self.members[index].clone()],
            DistributionInstruction::Claim { witness },
        )
    }

    fn claim(&mut self, index: usize) -> Result<(), Error> {
        let out = self.claim_with(index, self.witnesses[index].clone())?;
        self.apply(out, false);
        Ok(())
    }

    fn group_call(
        &mut self,
        member: Option<usize>,
        instruction: GroupInstruction,
    ) -> Result<(), Error> {
        let state_signed = matches!(instruction, GroupInstruction::Execute);
        let mut accounts = vec![self.state.clone()];
        if let Some(index) = member {
            accounts.push(self.members[index].clone());
        }
        let out = self.execute_group_checked(&accounts, instruction)?;
        self.apply(out, state_signed);
        Ok(())
    }

    fn propose(&mut self, index: usize, value: i64) -> Result<(), Error> {
        self.group_call(
            Some(index),
            GroupInstruction::Propose {
                witness: self.witnesses[index].clone(),
                next_value: value,
            },
        )
    }

    fn approve(&mut self, index: usize) -> Result<(), Error> {
        self.group_call(
            Some(index),
            GroupInstruction::Approve {
                witness: self.witnesses[index].clone(),
            },
        )
    }

    fn read_distribution(&self) -> Distribution {
        Distribution::try_from_slice(&self.state.account.data).unwrap()
    }

    fn read_group(&self) -> Group {
        Group::try_from_slice(&self.state.account.data).unwrap()
    }
}

#[test]
fn twenty_distinct_unit_claims_across_two_distributions() {
    // This is unit coverage, not the 20-claim TESTNET evidence required by LP-0003.
    for _ in 0..2 {
        let mut f = Fixture::new(10);
        f.distribution();
        for i in 0..10 {
            f.claim(i).unwrap();
        }
        assert_eq!(f.read_distribution().claims.len(), 10);
    }
}

#[test]
fn duplicate_claim_remains_denied_after_serialization_restart() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.claim(0).unwrap();
    let bytes = borsh::to_vec(&f.read_distribution()).unwrap();
    f.state.account.data = bytes.try_into().unwrap();
    assert_eq!(f.claim(0), Err(Error::DuplicateClaim));
}

#[test]
fn rejection_is_atomic_and_valid_retry_works() {
    let mut f = Fixture::new(3);
    f.distribution();
    let before = f.state.clone();
    let mut witness = f.witnesses[0].clone();
    witness.leaf.salt[0] ^= 1;
    assert_eq!(
        f.claim_with(0, witness).err(),
        Some(Error::InvalidMembership)
    );
    assert_eq!(f.state, before);
    f.claim(0).unwrap();
}

#[test]
fn changing_secret_does_not_create_another_claim_identity() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.claim(0).unwrap();
    let mut witness = f.witnesses[0].clone();
    witness.nullifier_secret_key[0] ^= 1;
    assert_eq!(
        f.claim_with(0, witness).err(),
        Some(Error::IdentityMismatch)
    );
}

#[test]
fn witness_cannot_be_used_by_another_private_account() {
    let mut f = Fixture::new(3);
    f.distribution();
    assert_eq!(
        f.claim_with(1, f.witnesses[0].clone()).err(),
        Some(Error::IdentityMismatch)
    );
}

#[test]
fn viewing_key_is_bound_to_actual_private_account() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut witness = f.witnesses[0].clone();
    witness.viewing_public_key = f.witnesses[1].viewing_public_key.clone();
    assert_eq!(
        f.claim_with(0, witness).err(),
        Some(Error::IdentityMismatch)
    );
}

#[test]
fn identifier_is_bound_to_actual_private_account() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut witness = f.witnesses[0].clone();
    witness.identifier += 1;
    assert_eq!(
        f.claim_with(0, witness).err(),
        Some(Error::IdentityMismatch)
    );
}

#[test]
fn known_witness_without_account_authorization_is_denied() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.members[0].is_authorized = false;
    assert_eq!(f.claim(0), Err(Error::Unauthorized));
}

#[test]
fn preexisting_member_owner_nonce_and_data_are_not_reassigned() {
    let mut f = Fixture::new(3);
    f.distribution();
    let out = f.claim_with(0, f.witnesses[0].clone()).unwrap();
    assert_eq!(out[1].account(), &f.members[0].account);
    assert!(out[1].required_claim().is_none());
    assert_eq!(out[1].account().nonce.0, 17);
}

#[test]
fn tampered_merkle_path_is_rejected() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.siblings[0][0] ^= 1;
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn wrong_path_length_is_rejected() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.siblings.pop();
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn oversized_path_is_rejected() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.siblings = vec![[0; 32]; 9];
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn padding_leaf_and_wrong_index_cannot_be_claimed() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.leaf_index = 3;
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn changing_entitlement_does_not_preserve_membership() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.leaf.entitlement = 2;
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn zero_entitlement_is_denied() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut w = f.witnesses[0].clone();
    w.leaf.entitlement = 0;
    assert_eq!(f.claim_with(0, w).err(), Some(Error::InvalidMembership));
}

#[test]
fn state_account_context_prevents_cross_distribution_replay() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.state.account_id = AccountId::new([43; 32]);
    assert_eq!(f.claim(0), Err(Error::InvalidMembership));
}

#[test]
fn program_context_prevents_cross_program_replay() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.program = [44; 8];
    f.state.account.program_owner = f.program;
    assert_eq!(f.claim(0), Err(Error::InvalidMembership));
}

#[test]
fn state_account_must_be_owned_by_current_program() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.state.account.program_owner = [45; 8];
    assert_eq!(f.claim(0), Err(Error::WrongOwner));
}

#[test]
fn corrupted_state_is_not_silently_reset() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.state.account.data = vec![1, 2, 3].try_into().unwrap();
    assert_eq!(f.claim(0), Err(Error::InvalidState));
}

#[test]
fn malformed_state_magic_is_rejected() {
    let mut f = Fixture::new(3);
    f.distribution();
    let mut state = f.read_distribution();
    state.magic = [0; 8];
    f.state.account.data = borsh::to_vec(&state).unwrap().try_into().unwrap();
    assert_eq!(f.claim(0), Err(Error::InvalidState));
}

#[test]
fn create_cannot_overwrite_existing_state() {
    let mut f = Fixture::new(3);
    f.distribution();
    let result = execute_distribution(
        f.program,
        &[f.state.clone()],
        DistributionInstruction::Create {
            root: f.root,
            member_count: 3,
        },
    );
    assert_eq!(result.err(), Some(Error::AlreadyInitialized));
}

#[test]
fn create_requires_authorization() {
    let mut f = Fixture::new(3);
    f.state.is_authorized = false;
    assert_eq!(
        execute_distribution(
            f.program,
            &[f.state.clone()],
            DistributionInstruction::Create {
                root: f.root,
                member_count: 3
            }
        )
        .err(),
        Some(Error::Unauthorized)
    );
}

#[test]
fn distribution_size_bounds_are_enforced() {
    let f = Fixture::new(3);
    for member_count in [0, 257, u32::MAX] {
        assert_eq!(
            execute_distribution(
                f.program,
                &[f.state.clone()],
                DistributionInstruction::Create {
                    root: f.root,
                    member_count
                }
            )
            .err(),
            Some(Error::InvalidSize)
        );
    }
}

#[test]
fn correct_account_count_is_required() {
    let f = Fixture::new(3);
    assert_eq!(
        execute_distribution(
            f.program,
            &[],
            DistributionInstruction::Create {
                root: f.root,
                member_count: 3
            }
        )
        .err(),
        Some(Error::AccountCount)
    );
    assert_eq!(
        execute_distribution(
            f.program,
            &[f.state.clone()],
            DistributionInstruction::Claim {
                witness: f.witnesses[0].clone()
            }
        )
        .err(),
        Some(Error::AccountCount)
    );
}

#[test]
fn single_member_tree_works_without_a_path() {
    let mut f = Fixture::new(1);
    f.distribution();
    assert!(f.witnesses[0].siblings.is_empty());
    f.claim(0).unwrap();
}

#[test]
fn full_supported_tree_can_claim_each_member_exactly_once() {
    let mut f = Fixture::new(256);
    f.distribution();
    for i in 0..256 {
        f.claim(i).unwrap();
    }
    assert_eq!(f.read_distribution().claims.len(), 256);
}

#[test]
fn merkle_builder_handles_every_supported_size() {
    for size in 1..=256 {
        let leaves: Vec<_> = (0..size)
            .map(|i| hash_parts(b"test", &[&(i as u64).to_le_bytes()]))
            .collect();
        let tree = merkle_tree(&leaves).unwrap();
        let expected_root = tree.last().unwrap()[0];
        for i in [0, size / 2, size - 1] {
            let (root, proof) = merkle_proof(&leaves, i).unwrap();
            assert_eq!(root, expected_root);
            assert!(proof.len() <= MAX_TREE_DEPTH);
        }
    }
}

#[test]
fn merkle_builder_rejects_empty_oversized_and_out_of_range() {
    assert_eq!(merkle_tree(&[]).err(), Some(Error::InvalidSize));
    assert_eq!(
        merkle_tree(&vec![[1; 32]; 257]).err(),
        Some(Error::InvalidSize)
    );
    assert_eq!(
        merkle_proof(&[[1; 32]], 1).err(),
        Some(Error::InvalidMembership)
    );
}

#[test]
fn hash_part_boundaries_are_unambiguous() {
    assert_ne!(
        hash_parts(b"d", &[b"ab", b"c"]),
        hash_parts(b"d", &[b"a", b"bc"])
    );
    assert_ne!(hash_parts(b"a", &[b"b"]), hash_parts(b"ab", &[]));
}

#[test]
fn public_distribution_data_contains_no_identity_or_witness_secret() {
    let mut f = Fixture::new(3);
    f.distribution();
    f.claim(0).unwrap();
    let data = f.state.account.data.as_ref();
    assert!(
        !data
            .windows(32)
            .any(|w| w == f.members[0].account_id.as_ref())
    );
    assert!(
        !data
            .windows(32)
            .any(|w| w == f.witnesses[0].nullifier_secret_key)
    );
    assert!(!data.windows(32).any(|w| w == f.witnesses[0].leaf.salt));
}

#[test]
fn two_of_three_parameter_update_requires_two_distinct_approvals() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 99).unwrap();
    assert_eq!(f.read_group().proposal.unwrap().approvals.len(), 0);
    f.approve(0).unwrap();
    assert_eq!(
        f.group_call(None, GroupInstruction::Execute),
        Err(Error::ThresholdNotMet)
    );
    f.approve(1).unwrap();
    f.group_call(None, GroupInstruction::Execute).unwrap();
    assert_eq!(f.read_group().value, 99);
    assert!(f.read_group().proposal.unwrap().executed);
}

#[test]
fn duplicate_vote_cannot_satisfy_threshold() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 10).unwrap();
    f.approve(0).unwrap();
    assert_eq!(f.approve(0), Err(Error::DuplicateApproval));
    assert_eq!(
        f.group_call(None, GroupInstruction::Execute),
        Err(Error::ThresholdNotMet)
    );
}

#[test]
fn partial_votes_survive_restart() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 25).unwrap();
    f.approve(0).unwrap();
    f.state.account.data = borsh::to_vec(&f.read_group()).unwrap().try_into().unwrap();
    f.approve(1).unwrap();
    f.group_call(None, GroupInstruction::Execute).unwrap();
    assert_eq!(f.read_group().value, 25);
}

#[test]
fn active_proposal_cannot_be_overwritten() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 25).unwrap();
    let before = f.state.clone();
    assert_eq!(f.propose(1, 999), Err(Error::PendingProposal));
    assert_eq!(f.state, before);
}

#[test]
fn approvals_cannot_be_added_after_threshold_or_execution() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 2).unwrap();
    f.approve(0).unwrap();
    f.approve(1).unwrap();
    assert_eq!(f.approve(2), Err(Error::ThresholdAlreadyMet));
    f.group_call(None, GroupInstruction::Execute).unwrap();
    assert_eq!(f.approve(2), Err(Error::AlreadyExecuted));
}

#[test]
fn execute_is_single_use() {
    let mut f = Fixture::new(1);
    f.group(1);
    f.propose(0, -5).unwrap();
    f.approve(0).unwrap();
    f.group_call(None, GroupInstruction::Execute).unwrap();
    let before = f.state.clone();
    assert_eq!(
        f.group_call(None, GroupInstruction::Execute),
        Err(Error::AlreadyExecuted)
    );
    assert_eq!(f.state, before);
}

#[test]
fn next_proposal_has_unlinkable_new_vote_nullifier() {
    let mut f = Fixture::new(1);
    f.group(1);
    f.propose(0, 1).unwrap();
    f.approve(0).unwrap();
    let first = f.read_group().proposal.unwrap().approvals[0];
    f.group_call(None, GroupInstruction::Execute).unwrap();
    f.propose(0, 2).unwrap();
    f.approve(0).unwrap();
    let second = f.read_group().proposal.unwrap().approvals[0];
    assert_ne!(first, second);
    assert_eq!(f.read_group().sequence, 2);
}

#[test]
fn approve_and_execute_require_a_proposal() {
    let mut f = Fixture::new(3);
    f.group(2);
    assert_eq!(f.approve(0), Err(Error::NoProposal));
    assert_eq!(
        f.group_call(None, GroupInstruction::Execute),
        Err(Error::NoProposal)
    );
}

#[test]
fn threshold_bounds_are_enforced() {
    let f = Fixture::new(3);
    for threshold in [0, 4, u32::MAX] {
        assert_eq!(
            execute_group(
                f.program,
                &[f.state.clone()],
                GroupInstruction::Create {
                    root: f.root,
                    member_count: 3,
                    threshold,
                    initial_value: 0
                }
            )
            .err(),
            Some(Error::InvalidThreshold)
        );
    }
}

#[test]
fn outsider_cannot_propose_or_approve() {
    let mut f = Fixture::new(3);
    f.group(2);
    let mut outsider = f.witnesses[0].clone();
    outsider.leaf.salt[0] ^= 1;
    assert_eq!(
        f.group_call(
            Some(0),
            GroupInstruction::Propose {
                witness: outsider.clone(),
                next_value: 0
            }
        ),
        Err(Error::InvalidMembership)
    );
    f.propose(0, 9).unwrap();
    assert_eq!(
        f.group_call(Some(0), GroupInstruction::Approve { witness: outsider }),
        Err(Error::InvalidMembership)
    );
}

#[test]
fn public_threshold_state_does_not_contain_member_addresses() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 9).unwrap();
    f.approve(0).unwrap();
    f.approve(1).unwrap();
    for member in &f.members {
        assert!(
            !f.state
                .account
                .data
                .windows(32)
                .any(|window| window == member.account_id.as_ref())
        );
    }
}

#[test]
fn stable_error_codes_and_messages_contain_no_secret_data() {
    assert_eq!(Error::DuplicateClaim as u32, 1009);
    assert_eq!(Error::DuplicateApproval as u32, 1014);
    assert_eq!(
        Error::ThresholdNotMet.to_string(),
        "COMMONS_ERROR_1015:ThresholdNotMet"
    );
}

#[test]
fn v024_create_output_claims_default_state_without_mutating_owner_or_nonce() {
    let f = Fixture::new(3);
    let posts = f
        .execute_distribution_checked(
            &[f.state.clone()],
            DistributionInstruction::Create {
                root: f.root,
                member_count: 3,
            },
        )
        .unwrap();
    assert_eq!(posts[0].required_claim(), Some(Claim::Authorized));
    assert_eq!(posts[0].account().program_owner, DEFAULT_PROGRAM_ID);
    assert_eq!(posts[0].account().nonce, Nonce::default());
    assert_ne!(posts[0].account().data, f.state.account.data);
}

#[test]
fn v024_create_rejects_non_default_balance_because_programs_never_move_tokens() {
    let mut f = Fixture::new(3);
    f.state.account.balance = 1;
    assert_eq!(
        execute_distribution(
            f.program,
            &[f.state.clone()],
            DistributionInstruction::Create {
                root: f.root,
                member_count: 3
            }
        )
        .err(),
        Some(Error::AlreadyInitialized)
    );
}

#[test]
fn v024_private_claim_output_preserves_public_state_nonce_and_member_account() {
    let mut f = Fixture::new(3);
    f.distribution();
    let state_nonce = f.state.account.nonce;
    let member_before = f.members[0].account.clone();
    let out = f.claim_with(0, f.witnesses[0].clone()).unwrap();
    assert_eq!(out[0].account().nonce, state_nonce);
    assert_eq!(out[1].account(), &member_before);
    assert!(out.iter().all(|post| post.required_claim().is_none()));
}

#[test]
fn v024_threshold_approve_preserves_member_account() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 10).unwrap();
    let accounts = [f.state.clone(), f.members[1].clone()];
    let posts = f
        .execute_group_checked(
            &accounts,
            GroupInstruction::Approve {
                witness: f.witnesses[1].clone(),
            },
        )
        .unwrap();
    assert_eq!(posts[1].account(), &f.members[1].account);
    assert!(posts[1].required_claim().is_none());
}

#[test]
fn viewing_public_key_bytes_match_upstream_private_account_id_derivation() {
    let nsk = [57_u8; 32];
    let npk = NullifierPublicKey::from(&nsk);
    let vpk = ViewingPublicKey::from_seed(&[1_u8; 32], &[2_u8; 32]);
    let vpk_bytes = ViewingPublicKeyBytes::from_viewing_public_key(&vpk);
    let identifier = 123_u128;

    let local = regular_private_account_id(&npk, &vpk_bytes, identifier).unwrap();
    let upstream = AccountId::for_regular_private_account(&npk, &vpk, identifier);

    assert_eq!(local, upstream);
}

#[test]
fn viewing_public_key_bytes_enforce_length_for_borsh_roundtrip() {
    let good = ViewingPublicKeyBytes::new(vec![7_u8; ViewingPublicKeyBytes::LEN]).unwrap();
    let encoded = borsh::to_vec(&good).unwrap();
    assert_eq!(
        ViewingPublicKeyBytes::try_from_slice(&encoded).unwrap(),
        good
    );

    let bad_vec_encoding = borsh::to_vec(&vec![7_u8; ViewingPublicKeyBytes::LEN - 1]).unwrap();
    assert!(ViewingPublicKeyBytes::try_from_slice(&bad_vec_encoding).is_err());
}

#[test]
fn same_member_can_claim_two_distributions_with_distinct_context_nullifiers() {
    let mut first = Fixture::new(1);
    first.distribution();
    first.claim(0).unwrap();
    let first_nullifier = first.read_distribution().claims[0];

    let mut second = Fixture::new(1);
    second.state.account_id = AccountId::new([77; 32]);
    let scope = context(second.program, second.state.account_id);
    let leaves = [second.witnesses[0].leaf.commitment(&scope)];
    second.root = merkle_proof(&leaves, 0).unwrap().0;
    second.witnesses[0].siblings = merkle_proof(&leaves, 0).unwrap().1;
    second.distribution();
    second.claim(0).unwrap();

    assert_ne!(first_nullifier, second.read_distribution().claims[0]);
}

#[test]
fn fresh_private_member_is_explicitly_claimed_before_nonce_rotation() {
    let mut f = Fixture::new(3);
    f.members[0].account = Account::default();
    f.group(2);
    let before = f.members[0].account.clone();
    let posts = f
        .execute_group_checked(
            &[f.state.clone(), f.members[0].clone()],
            GroupInstruction::Propose {
                witness: f.witnesses[0].clone(),
                next_value: 42,
            },
        )
        .unwrap();
    assert_eq!(posts[1].account(), &before);
    assert!(matches!(posts[1].required_claim(), Some(Claim::Authorized)));
}

#[test]
fn fresh_proposer_can_approve_after_realistic_private_nonce_rotation() {
    let mut f = Fixture::new(3);
    f.members[0].account = Account::default();
    f.group(2);
    let posts = f
        .execute_group_checked(
            &[f.state.clone(), f.members[0].clone()],
            GroupInstruction::Propose {
                witness: f.witnesses[0].clone(),
                next_value: 42,
            },
        )
        .unwrap();
    let mut member_after = posts[1].account().clone();
    assert!(matches!(posts[1].required_claim(), Some(Claim::Authorized)));
    // Mirror the protocol's claim assignment and private nonce rotation, not
    // merely the app-state update that missed this regression previously.
    member_after.program_owner = f.program;
    member_after.nonce = member_after
        .nonce
        .private_account_nonce_increment(&f.witnesses[0].nullifier_secret_key);
    f.members[0].account = member_after;
    f.apply(posts, false);
    let approved = f
        .execute_group_checked(
            &[f.state.clone(), f.members[0].clone()],
            GroupInstruction::Approve {
                witness: f.witnesses[0].clone(),
            },
        )
        .unwrap();
    assert_eq!(approved[1].account(), &f.members[0].account);
    assert!(approved[1].required_claim().is_none());
    f.apply(approved, false);
    assert_eq!(f.read_group().proposal.unwrap().approvals.len(), 1);
}

#[test]
fn fresh_allowlist_member_claim_assigns_owner_without_changing_balance() {
    let mut f = Fixture::new(1);
    f.members[0].account = Account::default();
    f.distribution();
    let posts = f.claim_with(0, f.witnesses[0].clone()).unwrap();
    assert!(matches!(posts[1].required_claim(), Some(Claim::Authorized)));
    assert_eq!(posts[1].account(), &Account::default());
}

#[test]
fn legacy_default_owner_with_rotated_nonce_is_rejected_without_state_changes() {
    let mut f = Fixture::new(1);
    f.distribution();
    f.members[0].account = Account::default();
    f.members[0].account.nonce = 7_u128.into();
    let before = f.state.account.data.clone();
    assert!(matches!(
        f.claim_with(0, f.witnesses[0].clone()),
        Err(Error::UninitializedMember)
    ));
    assert_eq!(f.state.account.data, before);
}

#[test]
fn completed_threshold_can_execute_without_creator_authorization() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 42).unwrap();
    f.approve(0).unwrap();
    f.approve(1).unwrap();
    f.state.is_authorized = false;
    let output = f
        .execute_group_checked(&[f.state.clone()], GroupInstruction::Execute)
        .unwrap();
    f.apply(output, false);
    let state = f.read_group();
    assert_eq!(state.value, 42);
    assert!(state.proposal.unwrap().executed);
}

#[test]
fn signature_free_executor_cannot_bypass_incomplete_threshold() {
    let mut f = Fixture::new(3);
    f.group(2);
    f.propose(0, 42).unwrap();
    f.approve(0).unwrap();
    f.state.is_authorized = false;
    let before = f.state.account.clone();
    assert!(matches!(
        f.execute_group_checked(&[f.state.clone()], GroupInstruction::Execute),
        Err(Error::ThresholdNotMet)
    ));
    assert_eq!(f.state.account, before);
}
