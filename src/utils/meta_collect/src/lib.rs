extern crate anyhow;
extern crate base64;
extern crate convert_case;
extern crate proc_macro;
extern crate quote;
extern crate serde_json;
extern crate syn;

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use convert_case::{Case, Casing};
use if_chain::if_chain;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use serde_json::{json, Value};
use std::{
    env,
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    FnArg, Ident, ItemFn, Lit, Pat, Token, Visibility,
};

struct MetaArgs {
    code: u64,
    name: String,
}

impl Parse for MetaArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let err_msg = format!(
            "expected `#[wgse_command(<code: u8>, <name: str>)]` at {:?}",
            input.span()
        );
        let input_args = Punctuated::<Lit, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect::<Vec<_>>();

        if_chain! {
            if input_args.len() == 2;
            if let Lit::Int(ref code) = input_args[0];
            if let Lit::Str(ref name) = input_args[1];
            then {
                return Ok(Self {
                    code: code.base10_parse::<u64>()?,
                    name: name.value(),
                });
            }
        };

        Err(input.error(err_msg))
    }
}

#[proc_macro_attribute]
pub fn wgse_command(args: TokenStream, input: TokenStream) -> TokenStream {
    let meta_args = parse_macro_input!(args as MetaArgs);
    let file_name = format!(
        "src/.autogen/wgse_commands/{}.json",
        meta_args.name.to_case(Case::Snake)
    );

    let out_dir = env::current_dir().expect("cannot find current dir");
    let dest_path = Path::new(&out_dir).join(&file_name);

    // Signature check
    let func = input.clone();
    let mut func = parse_macro_input!(func as ItemFn);
    let recv_ast = quote! { &self }.into();

    func.attrs = vec![];
    func.vis = Visibility::Inherited;
    func.sig
        .inputs
        .insert(0, parse_macro_input!(recv_ast as FnArg));
    func.sig.ident = Ident::new("_", Span::call_site());
    func.sig.inputs.iter_mut().for_each(|arg: &mut FnArg| {
        if_chain! {
            if let FnArg::Typed(arg) = arg;
            if let Pat::Ident(ref mut ident) = *arg.pat;
            then {
                ident.ident = Ident::new("_", Span::call_site());
            }
        }
    });

    let interface_path = Path::new(&out_dir).join("src/.autogen/interface.json");
    let interface = get_json_value(&interface_path).expect("Cannot get interface.");
    let sig = func.sig.clone();

    assert!(interface["raw"].is_string(), "no interface signature found, considered run `cargo build --features meta_init` before use this macro");

    let interface_str = interface["raw"].as_str().unwrap();
    let func_str = quote! { #sig; }.to_string();
    assert!(
        interface_str == func_str,
        "interface signature unconsistent, expected `{interface_str}`, found `{func_str}`"
    );

    let func = input.clone();
    let mut func = parse_macro_input!(func as ItemFn);
    let recv_ast = quote! { &self }.into();
    func.sig
        .inputs
        .insert(0, parse_macro_input!(recv_ast as FnArg));

    let autogen_codes = json!({
        "name": meta_args.name,
        "code": meta_args.code,
        "raw": quote! {#func}.to_string()
    });

    set_json_value(&dest_path, autogen_codes)
        .expect(&format!("cannot generate {}.json", meta_args.name));

    #[cfg(debug_assertions)]
    {
        return input;
    }

    #[cfg(not(debug_assertions))]
    {
        return TokenStream::new();
    }
}

fn get_json_value(path: &Path) -> Result<Value> {
    let mut content = String::new();
    BufReader::new(File::open(path)?).read_to_string(&mut content)?;

    let mut json_value: Value = serde_json::from_str(&content)?;
    if json_value["raw"].is_string() {
        json_value["raw"] = Value::String(String::from_utf8(
            general_purpose::STANDARD.decode(json_value["raw"].as_str().unwrap())?,
        )?);
    }

    Ok(json_value)
}

fn set_json_value(path: &Path, mut json_value: Value) -> Result<()> {
    if json_value["raw"].is_string() {
        json_value["raw"] =
            Value::String(general_purpose::STANDARD.encode(json_value["raw"].as_str().unwrap()))
    }

    BufWriter::new(File::create(path)?).write(json_value.to_string().as_bytes())?;
    Ok(())
}
