//! `#[derive(Schema)]`: be-wasm-schema's `Encode` and `Decode`, numbered by the crate's
//! `schema.lock`.
//!
//! The lockfile is read at `CARGO_MANIFEST_DIR` (buck passes every crate's `schema.lock` to
//! its compiles, buck/cargo/defs.bzl). The ids are the ones be-wasm-schema-lock's `numbered`
//! gives, which are the ones ./scripts/verify writes into the next lockfile, so a change
//! compiles before the lockfile is rewritten. A frozen type that changes incompatibly fails
//! here as well as in verify.

use be_wasm_schema_lock::{Entry, Lock, has_schema_word, numbered};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, FieldsNamed, Ident, parse_macro_input};

#[proc_macro_derive(Schema, attributes(schema))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(errors) => {
            let errors = errors
                .into_iter()
                .map(|error| syn::Error::new(Span::call_site(), error).to_compile_error());
            quote!(#(#errors)*).into()
        }
    }
}

fn read_lock() -> Result<Lock, Vec<String>> {
    let Ok(root) = std::env::var("CARGO_MANIFEST_DIR") else {
        return Ok(Lock::default());
    };
    let path = std::path::Path::new(&root).join(be_wasm_schema_lock::FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Lock::parse(&text).map_err(|error| vec![error]),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Lock::default()),
        Err(error) => Err(vec![format!("{}: {error}", path.display())]),
    }
}

fn expand(input: &DeriveInput) -> Result<TokenStream, Vec<String>> {
    let lock = read_lock()?;
    let entries = numbered(input, &lock)?;
    let entry = |name: &str| -> &Entry {
        &entries
            .iter()
            .find(|(described, _)| described.name == name)
            .expect("describe gives an entry for the type and each struct variant")
            .1
    };
    let name = &input.ident;
    let lifetime = input
        .generics
        .lifetimes()
        .next()
        .map(|param| &param.lifetime);
    let (generics, de) = match lifetime {
        Some(lifetime) => (quote!(<#lifetime>), quote!(#lifetime)),
        None => (quote!(), quote!('de)),
    };
    let decode_generics = match lifetime {
        Some(_) => generics.clone(),
        None => quote!(<'de>),
    };
    let ty = quote!(#name #generics);
    let type_name = name.to_string();
    let s = quote!(::be_wasm_schema);

    let mut checks = Vec::new();
    let body = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let entry = entry(&type_name);
                let message = Message::new(fields, entry, &type_name, &de, &mut checks);
                let encode = message.encode(&quote!(self.));
                let decode = message.decode(&quote!(Self));
                let absent = message.absent(&quote!(Self));
                let optional = message.optional();
                quote! {
                    impl #generics #s::Encode for #ty {
                        const WIRE: #s::Wire = #s::Wire::Len;
                        fn encode(&self, out: &mut ::std::vec::Vec<u8>) { #encode }
                    }
                    impl #decode_generics #s::Decode<#de> for #ty {
                        const OPTIONAL: bool = #optional;
                        fn decode(value: #s::Value<#de>) -> ::std::result::Result<Self, #s::Error> {
                            let _ = Self::__SCHEMA_CHECKS;
                            let mut reader = #s::Reader::of(value)?;
                            #decode
                        }
                        fn absent() -> ::std::option::Option<Self> { #absent }
                    }
                }
            }
            Fields::Unit => quote! {
                impl #s::Encode for #ty {
                    const WIRE: #s::Wire = #s::Wire::Len;
                    fn encode(&self, _: &mut ::std::vec::Vec<u8>) {}
                }
                impl<'de> #s::Decode<'de> for #ty {
                    const OPTIONAL: bool = true;
                    fn decode(value: #s::Value<'de>) -> ::std::result::Result<Self, #s::Error> {
                        let _ = Self::__SCHEMA_CHECKS;
                        let mut reader = #s::Reader::of(value)?;
                        while !reader.is_empty() {
                            #s::understood(reader.field()?.0)?;
                        }
                        Ok(Self)
                    }
                    fn absent() -> ::std::option::Option<Self> { Some(Self) }
                }
            },
            Fields::Unnamed(fields) => {
                let inner = &fields.unnamed[0].ty;
                quote! {
                    impl #generics #s::Encode for #ty {
                        const WIRE: #s::Wire = <#inner as #s::Encode>::WIRE;
                        fn encode(&self, out: &mut ::std::vec::Vec<u8>) { self.0.encode(out) }
                        fn is_absent(&self) -> bool { self.0.is_absent() }
                        fn encode_field(&self, key: #s::Key, out: &mut ::std::vec::Vec<u8>) {
                            self.0.encode_field(key, out)
                        }
                    }
                    impl #decode_generics #s::Decode<#de> for #ty {
                        const OPTIONAL: bool = <#inner as #s::Decode<#de>>::OPTIONAL;
                        fn decode(value: #s::Value<#de>) -> ::std::result::Result<Self, #s::Error> {
                            let _ = Self::__SCHEMA_CHECKS;
                            <#inner as #s::Decode<#de>>::decode(value).map(Self)
                        }
                        fn absent() -> ::std::option::Option<Self> {
                            <#inner as #s::Decode<#de>>::absent().map(Self)
                        }
                        fn decode_field(
                            value: ::std::option::Option<#s::Value<#de>>,
                            name: &'static str,
                        ) -> ::std::result::Result<Self, #s::Error> {
                            <#inner as #s::Decode<#de>>::decode_field(value, name).map(Self)
                        }
                    }
                }
            }
        },
        Data::Enum(data) => {
            let entry = entry(&type_name);
            let mut encode_arms = Vec::new();
            let mut decode_arms = Vec::new();
            let mut unknown_arm = quote! {
                _ => {
                    #s::understood(key)?;
                    Err(#s::Error::UnknownVariant(key.id))
                }
            };
            let mut default = None;
            for variant in &data.variants {
                let ident = &variant.ident;
                if has_schema_word(&variant.attrs, "unknown") {
                    encode_arms.push(quote!(Self::#ident(kept) => kept.write(out),));
                    unknown_arm =
                        quote!(_ => Ok(Self::#ident(#s::UnknownVariant::new(key, value)?)),);
                    continue;
                }
                if has_schema_word(&variant.attrs, "default") {
                    default = Some(ident);
                }
                let id = entry
                    .id(&ident.to_string())
                    .expect("every variant is numbered");
                let key = key(id, &variant.attrs);
                match &variant.fields {
                    Fields::Unit => {
                        encode_arms.push(quote! {
                            Self::#ident => #s::write_empty(#key, out),
                        });
                        decode_arms.push(quote!(#id => Ok(Self::#ident),));
                    }
                    Fields::Unnamed(fields) => {
                        let inner = &fields.unnamed[0].ty;
                        encode_arms
                            .push(quote!(Self::#ident(held) => #s::write_field(#key, held, out),));
                        decode_arms.push(quote! {
                            #id => <#inner as #s::Decode<#de>>::decode(value).map(Self::#ident),
                        });
                    }
                    Fields::Named(fields) => {
                        let of = format!("{type_name}::{ident}");
                        let message =
                            Message::new(fields, entry_named(&entries, &of), &of, &de, &mut checks);
                        let bindings = message.bindings();
                        let encode = message.encode(&quote!());
                        let decode = message.decode(&quote!(Self::#ident));
                        encode_arms.push(quote! {
                            Self::#ident { #(#bindings),* } => {
                                #s::write_key(#key, #s::Wire::Len, out);
                                #s::with_length(out, |out| { #encode });
                            }
                        });
                        decode_arms.push(quote! {
                            #id => { let mut reader = #s::Reader::of(value)?; #decode }
                        });
                    }
                }
            }
            let (optional, absent, is_absent) = match default {
                Some(ident) => (
                    quote!(true),
                    quote!(Some(Self::#ident)),
                    quote!(matches!(self, Self::#ident)),
                ),
                None => (quote!(false), quote!(None), quote!(false)),
            };
            quote! {
                impl #generics #s::Encode for #ty {
                    const WIRE: #s::Wire = #s::Wire::Len;
                    fn encode(&self, out: &mut ::std::vec::Vec<u8>) {
                        match self { #(#encode_arms)* }
                    }
                    fn is_absent(&self) -> bool { #is_absent }
                }
                impl #decode_generics #s::Decode<#de> for #ty {
                    const OPTIONAL: bool = #optional;
                    fn decode(value: #s::Value<#de>) -> ::std::result::Result<Self, #s::Error> {
                        let _ = Self::__SCHEMA_CHECKS;
                        let (key, value) = #s::variant(value)?;
                        match key.id {
                            #(#decode_arms)*
                            #unknown_arm
                        }
                    }
                    fn absent() -> ::std::option::Option<Self> { #absent }
                }
            }
        }
        Data::Union(_) => unreachable!("describe refuses unions"),
    };
    let decode_lifetime = match lifetime {
        Some(lifetime) => quote!(#lifetime),
        None => quote!('static),
    };
    let checks = checks.iter().map(
        |(ty, message)| quote!(assert!(<#ty as #s::Decode<#decode_lifetime>>::OPTIONAL, #message);),
    );
    Ok(quote! {
        #body
        impl #generics #ty {
            /// Every field added to a frozen type decodes when it is missing, as data an older
            /// build wrote leaves it.
            #[doc(hidden)]
            const __SCHEMA_CHECKS: () = { #(#checks)* };
        }
    })
}

fn entry_named<'a>(
    entries: &'a [(be_wasm_schema_lock::Described, Entry)],
    name: &str,
) -> &'a Entry {
    &entries
        .iter()
        .find(|(described, _)| described.name == name)
        .expect("describe gives an entry for each struct variant")
        .1
}

/// The key a member is written with: must-understand when it is `#[schema(critical)]`.
fn key(id: u32, attributes: &[syn::Attribute]) -> TokenStream {
    if has_schema_word(attributes, "critical") {
        quote!(::be_wasm_schema::Key::critical(#id))
    } else {
        quote!(::be_wasm_schema::Key::new(#id))
    }
}

struct Field<'a> {
    ident: &'a Ident,
    ty: &'a syn::Type,
    id: u32,
    key: TokenStream,
    slot: Ident,
    name: String,
}

/// The named fields of a struct or struct variant, by id.
struct Message<'a> {
    fields: Vec<Field<'a>>,
    unknown: Option<&'a Ident>,
    de: TokenStream,
}

impl<'a> Message<'a> {
    fn new(
        named: &'a FieldsNamed,
        entry: &Entry,
        of: &str,
        de: &TokenStream,
        checks: &mut Vec<(syn::Type, String)>,
    ) -> Self {
        let mut fields = Vec::new();
        let mut unknown = None;
        for field in &named.named {
            let ident = field.ident.as_ref().expect("named fields");
            if has_schema_word(&field.attrs, "unknown") {
                unknown = Some(ident);
                continue;
            }
            let id = entry
                .id(&ident.to_string())
                .expect("every field is numbered");
            if entry.frozen_from.is_some_and(|from| id >= from) {
                checks.push((
                    field.ty.clone(),
                    format!(
                        "{of}.{ident} was added to a frozen type, so it must decode when missing: make it an Option, a collection, a number, a string or another type with a default"
                    ),
                ));
            }
            fields.push(Field {
                ident,
                ty: &field.ty,
                id,
                key: key(id, &field.attrs),
                slot: format_ident!("__field_{}", id),
                name: format!("{of}.{ident}"),
            });
        }
        fields.sort_by_key(|field| field.id);
        Message {
            fields,
            unknown,
            de: de.clone(),
        }
    }

    fn bindings(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .map(|field| field.ident)
            .chain(self.unknown)
            .map(|ident| quote!(#ident))
            .collect()
    }

    /// Writes the fields by id, from `self.` (`access`) or from bindings (empty `access`),
    /// with the kept unknown fields among them.
    fn encode(&self, access: &TokenStream) -> TokenStream {
        let s = quote!(::be_wasm_schema);
        let writes = self.fields.iter().map(|field| {
            let (ident, id, key) = (field.ident, field.id, &field.key);
            let value = if access.is_empty() {
                quote!(#ident)
            } else {
                quote!(&#access #ident)
            };
            match self.unknown {
                Some(unknown) => quote! {
                    #access #unknown.write_below(#id, &mut __next, out);
                    #s::Encode::encode_field(#value, #key, out);
                },
                None => quote!(#s::Encode::encode_field(#value, #key, out);),
            }
        });
        match self.unknown {
            Some(unknown) => quote! {
                let mut __next = 0usize;
                #(#writes)*
                #access #unknown.write_below(u32::MAX, &mut __next, out);
            },
            None => quote!(#(#writes)*),
        }
    }

    /// Reads `reader`, inside the message, into `path { fields }`.
    fn decode(&self, path: &TokenStream) -> TokenStream {
        let s = quote!(::be_wasm_schema);
        let de = &self.de;
        let slots = self.fields.iter().map(|field| &field.slot);
        let arms = self.fields.iter().map(|field| {
            let (id, slot) = (field.id, &field.slot);
            quote!(#id => #s::once(&mut #slot, #id, value)?,)
        });
        let other = match self.unknown {
            Some(_) => quote!(_ => __unknown.keep(key, value)?,),
            None => quote!(_ => { #s::understood(key)?; }),
        };
        let values = self.fields.iter().map(|field| {
            let (ident, ty, slot, name) = (field.ident, field.ty, &field.slot, &field.name);
            quote!(#ident: <#ty as #s::Decode<#de>>::decode_field(#slot, #name)?,)
        });
        let unknown = self.unknown.map(|ident| quote!(#ident: __unknown,));
        let keeper = self
            .unknown
            .map(|_| quote!(let mut __unknown = #s::Unknown::default();));
        quote! {
            #(let mut #slots: ::std::option::Option<#s::Value<#de>> = None;)*
            #keeper
            while !reader.is_empty() {
                let (key, value) = reader.field()?;
                match key.id {
                    #(#arms)*
                    #other
                }
            }
            Ok(#path { #(#values)* #unknown })
        }
    }

    fn absent(&self, path: &TokenStream) -> TokenStream {
        let s = quote!(::be_wasm_schema);
        let de = &self.de;
        let values = self.fields.iter().map(|field| {
            let (ident, ty) = (field.ident, field.ty);
            quote!(#ident: <#ty as #s::Decode<#de>>::absent()?,)
        });
        let unknown = self
            .unknown
            .map(|ident| quote!(#ident: ::std::default::Default::default(),));
        quote!(Some(#path { #(#values)* #unknown }))
    }

    fn optional(&self) -> TokenStream {
        let s = quote!(::be_wasm_schema);
        let de = &self.de;
        let each = self.fields.iter().map(|field| {
            let ty = field.ty;
            quote!(<#ty as #s::Decode<#de>>::OPTIONAL)
        });
        quote!(true #(&& #each)*)
    }
}
