use proc_macro::TokenStream;
use quote::quote;
use syn::{
    FnArg, ItemTrait, LitBool, PatType, Path, TraitItem, TypeParamBound, parse_macro_input,
    parse_quote, spanned::Spanned,
};

// mostly AI generated, with small edits i made
// mix of chatgpt and gemini

#[proc_macro_attribute]
pub fn binder_ipc_object(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. Initialize a flag to hold our configuration state
    let mut is_root = false;
    let mut parent_trait = None;

    // 2. Parse the attribute helper arguments
    let attr_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("root") {
            // Match the `=` sign and parse the boolean literal (true/false)
            let value: LitBool = meta.value()?.parse()?;
            is_root = value.value();
            Ok(())
        } else {
            // Return an error if an unknown parameter is passed
            Err(meta.error("unsupported attribute property"))
        }
    });

    // Run the attribute parser against the raw `attr` stream
    parse_macro_input!(attr with attr_parser);

    // 3. Parse the target trait item
    let input_trait = parse_macro_input!(item as ItemTrait);
    let trait_name = &input_trait.ident;

    for bound in &input_trait.supertraits {
        if let TypeParamBound::Trait(trait_bound) = bound {
            if parent_trait.is_some() {
                return syn::Error::new(input_trait.span(), "Multiple super trait unsupported")
                    .to_compile_error()
                    .into();
            }

            // trait_bound.path gives you the full path of the supertrait
            parent_trait = Some(&trait_bound.path);
        }
    }

    let Some(parent_trait) = parent_trait else {
        return syn::Error::new(input_trait.span(), "Atleast one super trait is required")
            .to_compile_error()
            .into();
    };

    // 2. Extract the visibility status (e.g., `pub`, `pub(crate)`, or private)
    let trait_vis = &input_trait.vis;

    // 5. Combine the original trait with our generated extensions
    let snake_case_name = trait_name.to_string().to_lowercase();

    // Create a new valid identifier for a module name
    let mod_name = syn::Ident::new(&snake_case_name, trait_name.span());

    let mut current_transact_id: u32 = 0;
    let mut codes = Vec::new();

    let mut dispatchers = Vec::new();

    fn transform_path(path: &Path) -> Path {
        let mut new_path = path.clone();

        if let Some(last_segment) = new_path.segments.last_mut() {
            // Convert the last segment (e.g., "SuperTrait") to lowercase ("supertrait")
            let lowercase_ident = last_segment.ident.to_string().to_lowercase();
            last_segment.ident = syn::Ident::new(&lowercase_ident, last_segment.ident.span());
        }

        // Append the "abc" segment
        new_path.segments.push(parse_quote!(NEXT_TRANSACTION_CODE));

        new_path
    }
    let parent_trait_next_transact = transform_path(parent_trait);

    for item in &input_trait.items {
        if let TraitItem::Fn(method) = item {
            let name = &method.sig.ident;
            let code_name = syn::Ident::new(
                &format!("CODE_{}", name.to_string().to_uppercase()),
                name.span(),
            );

            if is_root {
                codes.push(quote! {
                    pub const #code_name: u32 = #current_transact_id;
                });
            } else {
                codes.push(quote! {
                    pub const #code_name: u32 = #parent_trait_next_transact + #current_transact_id;
                });
            }

            let mut args = Vec::new();

            // 1. Access the signature inputs
            for arg in &method.sig.inputs {
                match arg {
                    // Handles `self`, `&self`, `&mut self`, etc.
                    FnArg::Receiver(_) => {}

                    // Handles standard typed arguments (e.g., `name: Type`)
                    FnArg::Typed(PatType { pat, ty, .. }) => {
                        args.push(quote! {
                            <#ty as crate::packetable::Packetable>::deserialize(&mut msg_reader)
                                .context(concat!("Reading '", stringify!(#pat), "'"))?
                        });
                    }
                }
            }

            dispatchers.push(quote! {
                self::#code_name => {
                    target
                        .#name(#(#args),*)?
                        .serialize(&mut reply_writer)
                        .context("Cannot serializing return value")?;
                    Ok(reply_writer.finish())
                }
            });
            current_transact_id += 1;
        }
    }
    codes.push(quote! {
        pub static NEXT_TRANSACTION_CODE: u32 = #current_transact_id;
    });

    let fallback;

    if is_root {
        fallback = quote! { x => Err(anyhow!("unrecognized transaction code {x}")), };
    } else {
        fallback = quote! { _ => return <dyn #parent_trait>::decode_and_dispatch(target as &dyn #parent_trait, code, flags, message), };
    }

    TokenStream::from(quote! {
        #input_trait

        #trait_vis mod #mod_name {
            use crate::packetable::Packetable;
            use crate::{REPLY_SUCCESS, REPLY_FAILURE};
            use ::anyhow::{Context, anyhow, bail};

            // Want everything from super accessible here
            use super::*;
            #(#codes)*

            impl dyn #trait_name {
                pub fn decode_and_dispatch(
                    target: &dyn #trait_name,
                    code: ::std::primitive::u32,
                    flags: ::enumflags2::BitFlags<::libbinder::object::Flag>,
                    message: &mut ::libbinder::packet::Packet,
                ) -> ::std::result::Result<::std::option::Option<(::std::primitive::u32, ::libbinder::packet::Packet)>, ::libbinder::object::TransactionError> {
                    let reply = (|| {
                    let mut reply_writer = crate::writer::Writer::new(target.get_runtime());
                    let mut msg_reader = crate::reader::Reader::new(message);
                    match code {
                        #(#dispatchers),*
                        #fallback
                    }
                })();

                if flags.contains(::libbinder::object::Flag::OneWay) {
                    // Similar to Android's behaviour, don't send reply
                    Ok(None)
                } else {
                    match reply {
                        Ok(x) => Ok(Some((REPLY_SUCCESS, x))),
                        Err(err) => {
                            let mut writer = crate::writer::Writer::new(target.get_runtime());
                            err.serialize(&mut writer).unwrap();
                            Ok(Some((REPLY_FAILURE, writer.finish())))
                        }
                    }
                }
                }
            }
        }
    })
}
