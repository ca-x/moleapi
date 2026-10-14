// SPDX-License-Identifier: MPL-2.0
// MoleAPI compatibility extension; original emitter and public builder names are retained.
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{GenericArgument, Path, PathArguments, visit_mut::VisitMut};

pub(crate) fn qualify(code: TokenStream) -> TokenStream {
    let mut file: syn::File = syn::parse2(code).expect("Upstream builder tokens must parse");
    Qualify.visit_file_mut(&mut file);
    file.into_token_stream()
}

struct Qualify;
impl VisitMut for Qualify {
    fn visit_path_mut(&mut self, path: &mut Path) {
        syn::visit_mut::visit_path_mut(self, path);
        if path.leading_colon.is_some() {
            return;
        }
        let Some(first) = path.segments.first() else {
            return;
        };
        let lifetime_only = match &first.arguments {
            PathArguments::AngleBracketed(arguments) => {
                !arguments.args.is_empty()
                    && arguments
                        .args
                        .iter()
                        .all(|arg| matches!(arg, GenericArgument::Lifetime(_)))
            }
            _ => false,
        };
        if lifetime_only {
            return;
        } // Preserve impl Result<'a>/Option<'a>/... operation types.
        let name = first.ident.to_string();
        let target = match (name.as_str(), path.segments.len()) {
            ("Result", 1) => "::std::result::Result",
            ("Option", 1) => "::std::option::Option",
            ("String", 1) => "::std::string::String",
            ("Ok", 1) => "::std::result::Result::Ok",
            ("Err", 1) => "::std::result::Result::Err",
            ("Some", 1) => "::std::option::Option::Some",
            ("None", 1) => "::std::option::Option::None",
            ("Error", _) => "super::Error",
            ("ResponseValue", _) => "super::ResponseValue",
            ("ByteStream", 1) => "super::ByteStream",
            ("OperationInfo", 1) => "super::OperationInfo",
            _ => return,
        };
        let mut replacement: Path = syn::parse_str(target).expect("Static SDK path must parse");
        replacement.segments.last_mut().unwrap().arguments = first.arguments.clone();
        replacement
            .segments
            .extend(path.segments.iter().skip(1).cloned());
        *path = replacement;
    }
}
