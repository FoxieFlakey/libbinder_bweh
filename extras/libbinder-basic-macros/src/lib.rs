use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, ItemTrait, LitBool, LitStr, PatType, Path, PathSegment, ReturnType, TraitItem,
    TypeParamBound, parse_macro_input, parse_quote, spanned::Spanned,
};

// half AI generated, with small edits i made
// mix of chatgpt and gemini

#[proc_macro_attribute]
pub fn binder_ipc_object(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. Initialize a flag to hold our configuration state
    let mut is_root = false;
    let mut is_root_found = false;
    let mut interface_id = None;
    let mut parent_trait = None;

    // 2. Parse the attribute helper arguments
    let attr_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("root") {
            if is_root_found {
                return Err(meta.error("double 'root' attribute!"));
            }

            // Match the `=` sign and parse the boolean literal (true/false)
            let value: LitBool = meta.value()?.parse()?;
            is_root = value.value();
            is_root_found = true;
            Ok(())
        } else if meta.path.is_ident("interface_id") {
            if interface_id.is_some() {
                return Err(meta.error("double 'interface_id' attribute!"));
            }

            let value: LitStr = meta.value()?.parse()?;
            interface_id = Some(value.value());
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

    let Some(interface_id) = interface_id else {
        return syn::Error::new(input_trait.span(), "interface_id is required")
            .to_compile_error()
            .into();
    };

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

        new_path
    }

    fn append_segment(path: &Path, segment: PathSegment) -> Path {
        let mut ret = path.clone();
        ret.segments.push(segment);
        ret
    }

    let parent_module = transform_path(parent_trait);
    let parent_trait_next_transact =
        append_segment(&parent_module, parse_quote!(NEXT_TRANSACTION_CODE));

    let mut encoders = Vec::new();
    let mut forwarders = Vec::new();
    let mut forwarders_for_b = Vec::new();
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
            let mut arg_encoder_list = Vec::new();
            let mut arg_encoder = Vec::new();
            let mut arg_names = Vec::new();

            // 1. Access the signature inputs
            for arg in &method.sig.inputs {
                match arg {
                    // Handles `self`, `&self`, `&mut self`, etc.
                    FnArg::Receiver(_) => {}

                    // Handles standard typed arguments (e.g., `name: Type`)
                    FnArg::Typed(PatType { pat, ty, .. }) => {
                        arg_names.push(quote! {
                            #pat
                        });
                        args.push(quote! {
                            <#ty as libbinder_basic::packetable::Packetable>::deserialize(&mut msg_reader)
                                .context(concat!("Reading '", stringify!(#pat), "'"))?
                        });
                        arg_encoder_list.push(quote! {
                            #pat: #ty
                        });
                        arg_encoder.push(quote! {
                            Packetable::serialize(&#pat, &mut _ec27c490_2b98_48ab_a72e_ce7e50ff669e_writer)
                                .context(concat!("Cannot encode '", stringify!(#pat), "'"))?;
                        });
                    }
                }
            }

            dispatchers.push(quote! {
                self::#code_name => {
                    let ret = (|| -> anyhow::Result<()> {
                        Ok(target
                            .#name(#(#args),*)?
                            .serialize(&mut reply_writer)
                            .context("Cannot serializing return value")?)
                    })();
                    match ret {
                        Ok(()) => Ok(reply_writer.finish()),
                        Err(e) => Err(e),
                    }
                }
            });

            let ret = &method.sig.output;
            let return_type = match &method.sig.output {
                ReturnType::Default => quote! { () },

                ReturnType::Type(_, ty) => {
                    let syn::Type::Path(type_path) = &**ty else {
                        return syn::Error::new(ty.span(), "return type must be anyhow::Result<T>")
                            .to_compile_error()
                            .into();
                    };

                    let Some(segment) = type_path.path.segments.last() else {
                        return syn::Error::new(ty.span(), "invalid return type")
                            .to_compile_error()
                            .into();
                    };

                    if segment.ident != "Result" {
                        return syn::Error::new(
                            segment.span(),
                            "return type must be anyhow::Result<T>",
                        )
                        .to_compile_error()
                        .into();
                    }

                    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return syn::Error::new(
                            segment.span(),
                            "Result must have a type parameter",
                        )
                        .to_compile_error()
                        .into();
                    };

                    let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() else {
                        return syn::Error::new(args.span(), "could not extract Result<T>")
                            .to_compile_error()
                            .into();
                    };

                    quote! { #inner_ty }
                }
            };

            forwarders.push(quote! {
                fn #name(&self, #(#arg_encoder_list),*) #ret {
                    (&(self.$($target_field)*) as &dyn #trait_name).#name(#(#arg_names),*)
                }
            });

            forwarders_for_b.push(quote! {
                fn #name(&self, #(#arg_encoder_list),*) #ret {
                    use ::std::ops::Deref;
                    (self.deref() as &dyn #trait_name).#name(#(#arg_names),*)
                }
            });

            encoders.push(quote! {
                fn #name(&self, #(#arg_encoder_list),*) #ret {
                    // The giant uuid is just to avoid clashing when writing
                    let mut _ec27c490_2b98_48ab_a72e_ce7e50ff669e_writer = libbinder_basic::writer::Writer::new(self.get_runtime());
                    #(#arg_encoder);*
                    let mut packet = _ec27c490_2b98_48ab_a72e_ce7e50ff669e_writer.finish();
                    let (code, reply) = self
                        .on_transaction(#code_name, ::libbinder::object::Flag::AcceptFd.into(), &mut packet)
                        .context("Cannot perform transaction")?
                        .context("Expecting reply, but got none")?;

                    let mut reader = libbinder_basic::reader::Reader::new(&reply);
                    if code == libbinder_basic::REPLY_FAILURE {
                        // ABI for failure is just bare string
                        bail!(
                            "remote error: {}",
                            <&::std::primitive::str>::deserialize(&mut reader).context("Cannot read remote error message")?
                        );
                    } else if code == libbinder_basic::REPLY_SUCCESS {
                        // here you decode the resopne
                        Ok(<#return_type as libbinder_basic::packetable::Packetable>::deserialize(&mut reader).context("Cannot read remote's response")?)
                    } else {
                        bail!("Remote sent unknown reply code: {code}");
                    }
                }
            });
            current_transact_id += 1;
        }
    }
    if is_root {
        codes.push(quote! {
            pub const NEXT_TRANSACTION_CODE: u32 = #current_transact_id;
        });
    } else {
        codes.push(quote! {
            pub const NEXT_TRANSACTION_CODE: u32 = #parent_trait_next_transact + #current_transact_id;
        });
    }

    let proxy_name = format_ident!("Proxy{}", trait_name);
    let fallback;
    let proxy_forwarder;
    let base_proxy;
    let proxy_maker;
    let parent_forwarder;

    if is_root {
        fallback = quote! { x => Err(anyhow!("unrecognized transaction code {x}")), };
        proxy_forwarder = quote! {};
        parent_forwarder = quote! {};
        base_proxy = quote! { ::libbinder::proxy::Proxy };
        proxy_maker = quote! { Self { base } };
    } else {
        fallback = quote! { _ => return <dyn #parent_trait>::decode_and_dispatch(target as &dyn #parent_trait, code, flags, message), };
        let forwarder_macro = append_segment(&parent_module, parse_quote!(forwarder));
        proxy_forwarder = quote! {
            #forwarder_macro!(#proxy_name, base);
        };
        parent_forwarder = quote! {
            #forwarder_macro!($impl_name, $($target_field)*);
        };

        let base_proxy_name = format_ident!("Proxy{}", parent_trait.segments.last().unwrap().ident);
        let base_proxy_path = append_segment(&parent_module, parse_quote!(#base_proxy_name));
        base_proxy = quote! { #base_proxy_path };
        proxy_maker = quote! { Self { base: #base_proxy_path::new(base) } };
    }

    let proxy = quote! {
        pub struct #proxy_name {
            base: #base_proxy,
        }

        impl #proxy_name {
            pub fn new(base: ::libbinder::proxy::Proxy) -> Self {
                #proxy_maker
            }
        }

        impl ::libbinder::object::ObjectTrait for #proxy_name {
            fn get_remote<'a>(&'a self) -> ::std::option::Option<&'a ::libbinder::proxy::Proxy> {
                self.base.get_remote()
            }

            fn get_runtime(&self) -> ::std::sync::Arc<::libbinder::Runtime> {
                self.base.get_runtime()
            }

            fn on_transaction(
                &self,
                code: u32,
                flags: ::libbinder_basic::enumflags2::BitFlags<libbinder::object::Flag>,
                message: &mut ::libbinder::packet::Packet,
            ) -> Result<Option<(u32, ::libbinder::packet::Packet)>, ::libbinder::object::TransactionError> {
                self.base.on_transaction(code, flags, message)
            }
        }

        impl #trait_name for #proxy_name {
            #(#encoders)*
        }
    };

    TokenStream::from(quote! {
        #input_trait

        #trait_vis mod #mod_name {
            use libbinder_basic::packetable::Packetable;
            use libbinder_basic::{REPLY_SUCCESS, REPLY_FAILURE};
            use ::anyhow::{Context, anyhow, bail};

            // Want everything from super accessible here
            use super::*;
            #(#codes)*

            pub const ID: &str = #interface_id;

            impl dyn #trait_name {
                pub fn decode_and_dispatch(
                    target: &dyn #trait_name,
                    code: ::std::primitive::u32,
                    flags: ::libbinder_basic::enumflags2::BitFlags<::libbinder::object::Flag>,
                    message: &mut ::libbinder::packet::Packet,
                ) -> ::std::result::Result<::std::option::Option<(::std::primitive::u32, ::libbinder::packet::Packet)>, ::libbinder::object::TransactionError> {
                    let mut reply_writer = libbinder_basic::writer::Writer::new(target.get_runtime());
                    let mut msg_reader = libbinder_basic::reader::Reader::new(message);
                    let reply = match code {
                        #(#dispatchers),*
                        #fallback
                    };

                    if flags.contains(::libbinder::object::Flag::OneWay) {
                        // Similar to Android's behaviour, don't send reply
                        Ok(None)
                    } else {
                        match reply {
                            Ok(x) => Ok(Some((REPLY_SUCCESS, x))),
                            Err(err) => {
                                let mut writer = libbinder_basic::writer::Writer::new(target.get_runtime());
                                err.serialize(&mut writer).unwrap();
                                Ok(Some((REPLY_FAILURE, writer.finish())))
                            }
                        }
                    }
                }
            }

            #proxy
            #proxy_forwarder

            impl ::libbinder_basic::TryFromProxy for dyn #trait_name {
                type ProxyType = #proxy_name;

                fn try_from_proxy(proxy: ::libbinder::proxy::Proxy) -> ::std::result::Result<::std::sync::Arc<::libbinder::object::B<Self>>, ::libbinder::object::TransactionError> {
                    let temp = #proxy_name::new(proxy);
                    if !temp.has_interface(ID).map_err(|x| ::libbinder::object::TransactionError::Miscellanous(x.into()))? {
                        return Err(::libbinder::object::TransactionError::Miscellanous(anyhow!("Remote does not support '{}' interface", ID).into()));
                    }
                    Ok(::std::sync::Arc::new(::libbinder::object::B::new(temp)))
                }

                fn into_base(reference: ::std::sync::Arc<B<Self>>) -> ::std::sync::Arc<::libbinder::object::B<dyn ::libbinder::object::ObjectTrait>> {
                    reference
                }
            }

            macro_rules! forwarder {
                ($impl_name:ident, $($target_field:tt)*) => {
                    impl #trait_name for $impl_name {
                        #(#forwarders)*
                    }

                    #parent_forwarder
                };
            }

            macro_rules! decode_and_dispatch {
                ($impl_name:ident, $($target_field:tt)*) => {
                    impl ::libbinder::object::ObjectTrait for $impl_name {
                        fn get_remote<'a>(&'a self) -> std::option::Option<&'a ::libbinder::proxy::Proxy> {
                            self.base.get_remote()
                        }

                        fn get_runtime(&self) -> std::sync::Arc<::libbinder::Runtime> {
                            self.base.get_runtime()
                        }

                        fn on_transaction(
                            &self,
                            code: u32,
                            flags: ::libbinder_basic::enumflags2::BitFlags<::libbinder::object::Flag>,
                            message: &mut ::libbinder::packet::Packet,
                        ) -> ::std::result::Result<::std::option::Option<(u32, ::libbinder::packet::Packet)>, libbinder::object::TransactionError> {
                            <dyn #trait_name>::decode_and_dispatch(self, code, flags, message)
                        }
                    }
                };
            }

            pub(crate) use forwarder;
            pub(crate) use decode_and_dispatch;
        }
    })
}
