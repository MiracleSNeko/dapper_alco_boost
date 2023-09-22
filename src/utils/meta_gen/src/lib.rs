extern crate anyhow;
extern crate convert_case;
extern crate if_chain;
extern crate proc_macro;
extern crate quote;
extern crate serde_json;
extern crate syn;
extern crate walkdir;

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
    io::{prelude::*, BufReader, BufWriter},
    path::Path,
};
use syn::{
    parse, parse_macro_input, parse_str, FnArg, Ident, ItemEnum, ItemFn, Pat, TraitItemFn,
    Visibility,
};
use walkdir::WalkDir;

#[proc_macro_attribute]
pub fn wgse_command_interface(_: TokenStream, input: TokenStream) -> TokenStream {
    let out_dir = env::current_dir().expect("cannot open current dir");
    let dest_path = Path::new(&out_dir).join("src/.autogen/interface.json");

    let func = input.clone();
    let mut func = parse_macro_input!(func as TraitItemFn);

    func.attrs = vec![];
    func.sig.ident = Ident::new("_", Span::call_site());
    func.sig.inputs.iter_mut().for_each(|arg| {
        if_chain! {
            if let FnArg::Typed(arg) = arg;
            if let Pat::Ident(ref mut ident) = *arg.pat;
            then {
                ident.ident = Ident::new("_", Span::call_site());
            }
        }
    });

    set_json_value(&dest_path, json! {{"raw": quote! {#func}.to_string()}}).unwrap();
    input
}

#[proc_macro_attribute]
/// NOTE:
///     type of args and return is NOT full path.
pub fn generate_wgse_commands(arg: TokenStream, input: TokenStream) -> TokenStream {
    generate_wgse_commands_impl(arg, input).unwrap()
}

// Just for writing less annoying `unwrap` as much as possible.
fn generate_wgse_commands_impl(arg: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let out_dir = env::current_dir().expect("cannot find current dir");
    let dest_path = Path::new(&out_dir).join("src/.autogen/wgse_commands");
    let mut ast = TokenStream::new();
    let interface = parse::<Ident>(arg)?;
    let target_enum = parse::<ItemEnum>(input)?.ident;

    let mut tag_list = vec![];

    for entry in WalkDir::new(dest_path)
        .into_iter()
        .filter_map(|path| path.ok())
        .filter(|path| path.file_type().is_file())
    {
        let json_value = get_json_value(entry.path())?;

        // SAFETY: will NEVER panic at `unwrap`. Same as follows.
        let name = json_value["name"]
            .as_str()
            .unwrap()
            .to_case(Case::UpperCamel);
        let code = json_value["code"].as_u64().unwrap();
        let mut func = parse_str::<ItemFn>(json_value["raw"].as_str().unwrap())?;

        func.sig.ident = Ident::new("execute", Span::call_site());
        func.vis = Visibility::Inherited;

        // Enum element
        tag_list.push(name.to_string());

        // Command code constant
        let const_name = parse_str::<Ident>(&name.to_case(Case::UpperSnake))?;
        let const_ast: TokenStream = quote! {
            const #const_name: u64 = #code;
        }
        .into();

        // Enum tag
        let name = parse_str::<Ident>(&name)?;
        let enum_tag_ast = quote! {
            #[derive(Debug, Default, Clone, Eq, PartialEq)]
            pub struct #name;
        }
        .into();

        // Implemention block for enum_dispatch
        let impl_ast = quote! {
            impl #interface for #name {
                #func
            }
        }
        .into();

        ast.extend(vec![const_ast, enum_tag_ast, impl_ast]);
    }

    let tags = tag_list.into_iter().map(|tag| {
        let tag = parse_str::<Ident>(&tag).unwrap();
        quote! {
            #tag,
        }
    });

    // Note: the default command MUST be `.Nope`
    let generated_enum: TokenStream = quote! {
        #[enum_dispatch(#interface)]
        #[derive(Debug, Clone, Eq, PartialEq)]
        pub enum #target_enum
        {
            #(#tags)*
        }

        impl std::default::Default for #target_enum {
            fn default() -> Self {
                #target_enum::Nope(Nope{})
            }
        }
    }
    .into();

    ast.extend(vec![generated_enum]);
    Ok(ast)
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
