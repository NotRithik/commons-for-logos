//! Separate source-derived SPEL interface for the composable consumer example.
use super::ty;
use anyhow::{bail, ensure, Context, Result};
use quote::ToTokens;
use spel_framework_core::{idl::*, idl_gen::generate_idl_from_file};
use std::{fs, path::Path};

pub fn generate(root: &Path, out: &Path) -> Result<()> {
    let ast = syn::parse_file(&fs::read_to_string(
        root.join("testnet/crates/adapter/src/lib.rs"),
    )?)?;
    let instruction = ast
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == "GovernedSettingInstruction" => Some(e),
            _ => None,
        })
        .context("Adapter instruction enum missing")?;
    let error_enum = ast
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == "AdapterError" => Some(e),
            _ => None,
        })
        .context("Adapter error enum missing")?;
    let mut defs = Vec::new();
    for item in &ast.items {
        if let syn::Item::Struct(s) = item {
            if ["PolicyBinding", "GovernedSetting"].contains(&s.ident.to_string().as_str()) {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| {
                        Ok(IdlField {
                            name: f
                                .ident
                                .as_ref()
                                .context("Named adapter field required")?
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
    let mut decl = String::from("// GENERATED IDL declaration, NOT executable guest code.\n#[lez_program(instruction = \"commons_logos_policy_adapter::GovernedSettingInstruction\")]\nmod commons_governed_setting_v024 {\n");
    for variant in &instruction.variants {
        let attr = if variant.ident == "Initialize" {
            "mut, init, signer"
        } else {
            "mut"
        };
        decl.push_str(&format!(" #[instruction]\n pub fn {}(\n #[account({attr})] consumer: AccountWithMetadata,\n #[account()] policy_state: AccountWithMetadata,\n",variant.ident.to_string().to_lowercase()));
        for field in &variant.fields {
            decl.push_str(&format!(
                " {}: {},\n",
                field
                    .ident
                    .as_ref()
                    .context("Named instruction argument required")?,
                field.ty.to_token_stream()
            ));
        }
        decl.push_str(") {}\n");
    }
    decl.push_str("}\n");
    let path = out.join("commons_governed_setting_v024.contract.rs");
    fs::write(&path, decl)?;
    let mut idl =
        generate_idl_from_file(&path).map_err(|e| anyhow::anyhow!("SPEL adapter parser: {e:?}"))?;
    ensure!(
        idl.instructions.len() == instruction.variants.len(),
        "Adapter IDL instruction mismatch"
    );
    for (ix, variant) in idl.instructions.iter_mut().zip(instruction.variants.iter()) {
        ix.variant = Some(variant.ident.to_string());
        ix.discriminator = None;
        ix.execution = Some(IdlExecution {
            public: true,
            private_owned: false,
        });
        ix.args = variant
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
            a.visibility = vec!["public".into()];
        }
    }
    idl.errors = error_enum
        .variants
        .iter()
        .map(|v| {
            let (_, expr) = v
                .discriminant
                .as_ref()
                .context("Explicit adapter error required")?;
            let code = match expr {
                syn::Expr::Lit(l) => match &l.lit {
                    syn::Lit::Int(n) => n.base10_parse()?,
                    _ => bail!("Non-integer adapter error"),
                },
                _ => bail!("Invalid adapter error"),
            };
            Ok(IdlError {
                code,
                name: v.ident.to_string(),
                msg: Some(format!("COMMONS_ADAPTER_{code}_{}", v.ident)),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    idl.accounts = vec![IdlAccountType {
        name: "GovernedSetting".into(),
        type_: defs
            .iter()
            .find(|d| d.name == "GovernedSetting")
            .context("Consumer state type")?
            .clone(),
    }];
    idl.types = defs;
    idl.metadata = Some(IdlMetadata {
        name: "commons_governed_setting_v024".into(),
        version: "0.1.0".into(),
    });
    fs::write(
        out.join("commons_governed_setting_v024.json"),
        idl.to_json_pretty()? + "\n",
    )?;
    Ok(())
}
