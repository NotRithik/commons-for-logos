//! Accurate v0.2.4 IDL generation through the pinned official SPEL parser.
//! Interface declarations are derived from the compiled program's real enum.
use anyhow::{bail, ensure, Context, Result};
use quote::ToTokens;
use serde_json::json;
use spel_framework_core::{idl::*, idl_gen::generate_idl_from_file};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

fn ty(t: &syn::Type) -> Result<IdlType> {
    Ok(match t {
        syn::Type::Array(a) => {
            let n = match &a.len {
                syn::Expr::Lit(l) => {
                    if let syn::Lit::Int(i) = &l.lit {
                        i.base10_parse::<usize>()?
                    } else {
                        bail!("non-integer array length")
                    }
                }
                _ => bail!("nonliteral array length"),
            };
            IdlType::Array {
                array: (Box::new(ty(&a.elem)?), n),
            }
        }
        syn::Type::Path(p) => {
            let s = p.path.segments.last().context("empty type")?;
            let name = s.ident.to_string();
            match name.as_str() {
                "Hash32" => IdlType::Array {
                    array: (Box::new(IdlType::Primitive("u8".into())), 32),
                },
                "ProgramId" => IdlType::Array {
                    array: (Box::new(IdlType::Primitive("u32".into())), 8),
                },
                "AccountId" => IdlType::Primitive("pubkey".into()),
                "ViewingPublicKeyBytes" => IdlType::Vec {
                    vec: Box::new(IdlType::Primitive("u8".into())),
                },
                "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
                | "bool" => IdlType::Primitive(name),
                "Vec" | "Option" => {
                    let arg = match &s.arguments {
                        syn::PathArguments::AngleBracketed(a) => {
                            a.args.first().context("missing generic")?
                        }
                        _ => bail!("missing generic"),
                    };
                    let elem = match arg {
                        syn::GenericArgument::Type(t) => ty(t)?,
                        _ => bail!("unsupported generic"),
                    };
                    if name == "Vec" {
                        IdlType::Vec {
                            vec: Box::new(elem),
                        }
                    } else {
                        IdlType::Option {
                            option: Box::new(elem),
                        }
                    }
                }
                _ => IdlType::Defined { defined: name },
            }
        }
        _ => bail!("unsupported type {}", t.to_token_stream()),
    })
}
fn generate(root: &Path, out: &Path) -> Result<Vec<SpelIdl>> {
    fs::create_dir_all(out)?;
    let text = fs::read_to_string(root.join("testnet/crates/primitives/src/lib.rs"))?;
    let ast = syn::parse_file(&text)?;
    let enums: HashMap<String, &syn::ItemEnum> = ast
        .items
        .iter()
        .filter_map(|x| {
            if let syn::Item::Enum(e) = x {
                Some((e.ident.to_string(), e))
            } else {
                None
            }
        })
        .collect();
    let wanted = [
        "MemberLeaf",
        "MemberWitness",
        "Distribution",
        "Proposal",
        "Group",
    ];
    let mut defs = Vec::new();
    for item in &ast.items {
        if let syn::Item::Struct(s) = item {
            if wanted.contains(&s.ident.to_string().as_str()) {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| {
                        Ok(IdlField {
                            name: f
                                .ident
                                .as_ref()
                                .context("named struct field required")?
                                .to_string(),
                            type_: ty(&f.ty)?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                defs.push(IdlTypeDef {
                    name: s.ident.to_string(),
                    kind: "struct".into(),
                    fields,
                    variants: vec![],
                });
            }
        }
    }
    let error_enum = enums.get("Error").context("Error enum missing")?;
    let errors = error_enum
        .variants
        .iter()
        .map(|v| {
            let (_, e) = v
                .discriminant
                .as_ref()
                .context("explicit error code required")?;
            let code = match e {
                syn::Expr::Lit(l) => {
                    if let syn::Lit::Int(n) = &l.lit {
                        n.base10_parse()?
                    } else {
                        bail!("invalid error code")
                    }
                }
                _ => bail!("invalid code"),
            };
            Ok(IdlError {
                code,
                name: v.ident.to_string(),
                msg: Some(format!("ASTRA_ERROR_{code}:{}", v.ident)),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut result = Vec::new();
    for (enum_name, module, state_type) in [
        (
            "DistributionInstruction",
            "astra_allowlist_v024",
            "Distribution",
        ),
        ("GroupInstruction", "astra_threshold_v024", "Group"),
    ] {
        let en = enums.get(enum_name).context("instruction enum missing")?;
        let mut decl=format!("// GENERATED IDL declaration, NOT executable guest code.\n#[lez_program(instruction = \"astra_logos_testnet_primitives::{enum_name}\")]\nmod {module} {{\n");
        for v in &en.variants {
            let variant = v.ident.to_string();
            let private = v
                .fields
                .iter()
                .any(|f| f.ident.as_ref().is_some_and(|n| n == "witness"));
            decl.push_str(" #[instruction]\n pub fn ");
            decl.push_str(&variant.to_lowercase());
            decl.push_str("(\n");
            let attr = if variant == "Create" {
                "mut, init, signer"
            } else {
                "mut"
            };
            decl.push_str(&format!(
                " #[account({attr})] state: AccountWithMetadata,\n"
            ));
            if private {
                decl.push_str(" #[account(signer)] member: AccountWithMetadata,\n");
            }
            for f in &v.fields {
                decl.push_str(&format!(
                    " {}: {},\n",
                    f.ident.as_ref().context("named variant field required")?,
                    f.ty.to_token_stream()
                ));
            }
            decl.push_str(") {}\n");
        }
        decl.push_str("}\n");
        let source = out.join(format!("{module}.contract.rs"));
        fs::write(&source, decl)?;
        // The official parser produces the accounts, instruction names and ordering.
        let mut idl = generate_idl_from_file(&source)
            .map_err(|e| anyhow::anyhow!("SPEL IDL parser: {e:?}"))?;
        ensure!(
            idl.instructions.len() == en.variants.len(),
            "SPEL instruction count mismatch"
        );
        for (ix, v) in idl.instructions.iter_mut().zip(en.variants.iter()) {
            let private = v
                .fields
                .iter()
                .any(|f| f.ident.as_ref().is_some_and(|n| n == "witness"));
            ix.variant = Some(v.ident.to_string());
            ix.discriminator = None;
            ix.execution = Some(IdlExecution {
                public: !private,
                private_owned: private,
            });
            ix.args = v
                .fields
                .iter()
                .map(|f| {
                    Ok(IdlArg {
                        name: f.ident.as_ref().context("field")?.to_string(),
                        type_: ty(&f.ty)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            for a in &mut ix.accounts {
                a.visibility = vec![if a.name == "member" {
                    "private".into()
                } else {
                    "public".into()
                }];
            }
        }
        idl.types = defs.clone();
        idl.errors = errors.clone();
        idl.accounts = vec![IdlAccountType {
            name: state_type.into(),
            type_: defs
                .iter()
                .find(|x| x.name == state_type)
                .context("state type")?
                .clone(),
        }];
        idl.metadata = Some(IdlMetadata {
            name: module.into(),
            version: "0.1.0".into(),
        });
        fs::write(
            out.join(format!("{module}.json")),
            idl.to_json_pretty()? + "\n",
        )?;
        result.push(idl);
    }
    fs::write(
        out.join("wire-format.json"),
        serde_json::to_vec_pretty(&json!({
         "protocol":"LEZ v0.2.4","lez_revision":"47eba256479f6f785acbd138834340703cd03401",
         "spel_revision":"5126b7ed8a9b78d4c666073fa91a66713c5f2a2b",
         "instruction_encoding":"risc0_zkvm::serde Vec<u32>; enum tag is its zero-based declaration-order u32, not SHA8",
         "state_encoding":"borsh v1; state magic is the first 8 bytes",
         "viewing_public_key":"Vec<u8> constrained to exactly 1184 bytes; not a variable-length arbitrary key",
         "private_only":["allowlist.Claim","threshold.Propose","threshold.Approve"],
         "warning":"Private instructions carry secret witnesses. Never submit their bytes as public transactions or publish an inner guest journal."
        }))?,
    )?;
    Ok(result)
}
fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let out = root.join("idl/generated");
    let idls = generate(&root, &out)?;
    println!(
        "Generated {} exact v0.2.4 SPEL IDLs at {}",
        idls.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use astra_logos_testnet_primitives::{DistributionInstruction, GroupInstruction};
    #[test]
    fn generated_order_privacy_and_no_fictional_discriminators() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        let out = root.join("idl/target/idl-test-output");
        let idls = generate(&root, &out)?;
        assert_eq!(
            idls[0]
                .instructions
                .iter()
                .map(|i| i.variant.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["Create", "Claim"]
        );
        assert_eq!(
            idls[1]
                .instructions
                .iter()
                .map(|i| i.variant.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["Create", "Propose", "Approve", "Execute"]
        );
        for idl in idls {
            for i in idl.instructions {
                assert!(i.discriminator.is_none());
                let private = i.args.iter().any(|x| x.name == "witness");
                let e = i.execution.unwrap();
                assert_eq!(e.private_owned, private);
                assert_eq!(e.public, !private);
            }
        }
        Ok(())
    }
    #[test]
    fn actual_guest_instruction_tags_and_roundtrip() {
        let create = DistributionInstruction::Create {
            root: [1; 32],
            member_count: 10,
        };
        let data = risc0_zkvm::serde::to_vec(&create).unwrap();
        assert_eq!(data[0], 0);
        let _: DistributionInstruction = risc0_zkvm::serde::from_slice(&data).unwrap();
        let execute = risc0_zkvm::serde::to_vec(&GroupInstruction::Execute).unwrap();
        assert_eq!(execute, vec![3]);
        assert!(risc0_zkvm::serde::from_slice::<GroupInstruction, u32>(&[4]).is_err());
    }
    #[test]
    fn source_derived_types_are_complete_and_errors_unique() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        let idls = generate(&root, &root.join("idl/target/idl-test-output2"))?;
        for idl in idls {
            let names: Vec<_> = idl.types.iter().map(|x| x.name.as_str()).collect();
            for n in [
                "MemberLeaf",
                "MemberWitness",
                "Distribution",
                "Proposal",
                "Group",
            ] {
                assert!(names.contains(&n));
            }
            let mut codes: Vec<_> = idl.errors.iter().map(|e| e.code).collect();
            let original = codes.len();
            codes.sort();
            codes.dedup();
            assert_eq!(codes.len(), original);
            assert_eq!(codes[0], 1001);
            assert_eq!(*codes.last().unwrap(), 1021);
            let json = idl.to_json_pretty()?;
            let _: SpelIdl = serde_json::from_str(&json)?;
        }
        Ok(())
    }
}
