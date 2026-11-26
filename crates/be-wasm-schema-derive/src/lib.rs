//! `#[derive(Schema)]`: be-wasm-schema's `Encode` and `Decode`, numbered by the crate's
//! `schema.lock`.
//!
//! The lockfile is read at `CARGO_MANIFEST_DIR` (buck passes every crate's `schema.lock` to
//! its compiles, buck/cargo/defs.bzl). The ids are the ones be-wasm-schema-lock's `numbered`
//! gives, which are the ones ./scripts/verify writes into the next lockfile, so a change
//! compiles before the lockfile is rewritten. A frozen type that changes incompatibly fails
//! here as well as in verify, and two checks are the compiler's, as associated consts the
//! encode and decode functions force: that a frozen type holds only frozen types
//! (`Encode::FROZEN`), and that a field added to it after freezing decodes when missing.

use be_wasm_schema_lock::{Described, Entry, Lock, has_schema_word, numbered, schema_default};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Fields, FieldsNamed, GenericParam, Ident, Lifetime, parse_macro_input,
    parse_quote,
};

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

/// The generics of the two impls: every type parameter bound by `Encode`, or by `Decode<'de>`
/// with `'de` the type's own lifetime or one added.
struct Generics {
    encode: syn::Generics,
    decode: syn::Generics,
    de: Lifetime,
    /// Whether the type has type parameters, so its checks are forced only where it is used.
    generic: bool,
}

impl Generics {
    fn of(input: &DeriveInput) -> Self {
        let mut encode = input.generics.clone();
        for param in encode.type_params_mut() {
            param.bounds.push(parse_quote!(::be_wasm_schema::Encode));
        }
        let own = input
            .generics
            .lifetimes()
            .next()
            .map(|param| param.lifetime.clone());
        let de = own
            .clone()
            .unwrap_or_else(|| Lifetime::new("'de", Span::call_site()));
        let mut decode = input.generics.clone();
        for param in decode.type_params_mut() {
            param
                .bounds
                .push(parse_quote!(::be_wasm_schema::Decode<#de>));
        }
        if own.is_none() {
            decode
                .params
                .insert(0, GenericParam::Lifetime(parse_quote!(#de)));
        }
        Generics {
            encode,
            decode,
            de,
            generic: input.generics.type_params().next().is_some(),
        }
    }
}

fn expand(input: &DeriveInput) -> Result<TokenStream, Vec<String>> {
    let lock = read_lock()?;
    let entries = numbered(input, &lock)?;
    let s = quote!(::be_wasm_schema);
    let name = &input.ident;
    let type_name = name.to_string();
    let frozen = entries[0].0.frozen;
    let generics = Generics::of(input);
    let de = &generics.de;
    let (encode_impl, _, encode_where) = generics.encode.split_for_impl();
    let (decode_impl, _, decode_where) = generics.decode.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();
    let ty = quote!(#name #ty_generics);

    let mut checks = Checks::default();
    let (wire, encode_body, extra_encode, decode_body, extra_decode) = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let message = Message::new(
                    fields,
                    entry(&entries, &type_name),
                    &type_name,
                    de,
                    frozen,
                    &mut checks,
                );
                let encode = message.encode(&quote!(self.));
                let decode = message.decode(&quote!(Self));
                let absent = message.absent(&quote!(Self));
                let optional = message.optional();
                (
                    quote!(#s::Wire::Message),
                    quote!(#encode Ok(())),
                    quote!(),
                    quote!(let mut reader = #s::Reader::of(value)?; #decode),
                    quote! {
                        const OPTIONAL: bool = #optional;
                        fn absent() -> ::std::option::Option<Self> { #absent }
                    },
                )
            }
            Fields::Unit => (
                quote!(#s::Wire::Message),
                quote!(Ok(())),
                quote!(),
                quote! {
                    let mut reader = #s::Reader::of(value)?;
                    while !reader.is_empty() {
                        #s::understood(reader.field()?.0)?;
                    }
                    Ok(Self)
                },
                quote! {
                    const OPTIONAL: bool = true;
                    fn absent() -> ::std::option::Option<Self> { Some(Self) }
                },
            ),
            Fields::Unnamed(fields) => {
                let inner = &fields.unnamed[0].ty;
                if frozen {
                    checks.frozen(inner, &type_name);
                }
                (
                    quote!(<#inner as #s::Encode>::WIRE),
                    quote!(#s::Encode::encode(&self.0, w)),
                    quote! {
                        fn is_absent(&self) -> bool { #s::Encode::is_absent(&self.0) }
                        fn encode_field(
                            &self,
                            key: #s::Key,
                            w: &mut #s::Writer,
                        ) -> ::std::result::Result<(), #s::Error> {
                            #s::Encode::encode_field(&self.0, key, w)
                        }
                    },
                    quote!(<#inner as #s::Decode<#de>>::decode(value).map(Self)),
                    quote! {
                        const OPTIONAL: bool = <#inner as #s::Decode<#de>>::OPTIONAL;
                        fn absent() -> ::std::option::Option<Self> {
                            <#inner as #s::Decode<#de>>::absent().map(Self)
                        }
                        fn decode_field(
                            value: ::std::option::Option<#s::Value<#de>>,
                        ) -> ::std::result::Result<Self, #s::Error> {
                            <#inner as #s::Decode<#de>>::decode_field(value).map(Self)
                        }
                    },
                )
            }
        },
        Data::Enum(data) => {
            let entry = entry(&entries, &type_name);
            let mut encode_arms = Vec::new();
            let mut decode_arms = Vec::new();
            let mut unknown_arm = quote! {
                _ => {
                    #s::understood(key)?;
                    Err(#s::Error::new(#s::ErrorKind::UnknownVariant(key.id)))
                }
            };
            let mut default = None;
            for variant in &data.variants {
                let ident = &variant.ident;
                if has_schema_word(&variant.attrs, "unknown") {
                    encode_arms.push(quote!(Self::#ident(kept) => kept.write(w),));
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
                let within = format!("{type_name}::{ident}");
                match &variant.fields {
                    Fields::Unit => {
                        encode_arms.push(quote!(Self::#ident => { w.empty(#key); Ok(()) }));
                        decode_arms.push(quote!(#id => Ok(Self::#ident),));
                    }
                    Fields::Unnamed(fields) => {
                        let inner = &fields.unnamed[0].ty;
                        if frozen {
                            checks.frozen(inner, &within);
                        }
                        encode_arms.push(quote!(Self::#ident(held) => w.field(#key, held),));
                        decode_arms.push(quote! {
                            #id => Ok(Self::#ident(
                                <#inner as #s::Decode<#de>>::decode(value)
                                    .map_err(|error| error.within(#within))?,
                            )),
                        });
                    }
                    Fields::Named(fields) => {
                        let message = Message::new(
                            fields,
                            entry_named(&entries, &within),
                            &within,
                            de,
                            frozen,
                            &mut checks,
                        );
                        let bindings = message.bindings();
                        let encode = message.encode(&quote!());
                        let decode = message.decode(&quote!(Self::#ident));
                        encode_arms.push(quote! {
                            Self::#ident { #(#bindings),* } => w.message(#key, |w| { #encode Ok(()) }),
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
            (
                quote!(#s::Wire::Message),
                quote!(match self { #(#encode_arms)* }),
                quote!(fn is_absent(&self) -> bool { #is_absent }),
                quote! {
                    let (key, value) = #s::variant(value)?;
                    match key.id {
                        #(#decode_arms)*
                        #unknown_arm
                    }
                },
                quote! {
                    const OPTIONAL: bool = #optional;
                    fn absent() -> ::std::option::Option<Self> { #absent }
                },
            )
        }
        Data::Union(_) => unreachable!("describe refuses unions"),
    };
    let frozen_checks = &checks.frozen;
    let optional_checks = checks
        .optional
        .iter()
        .map(|(ty, message)| quote!(assert!(<#ty as #s::Decode<#de>>::OPTIONAL, #message);));
    // A type with no type parameters is checked wherever it is defined; a generic one where it
    // is used, as each use may hold different types.
    let forced = if generics.generic {
        quote!()
    } else {
        let lifetime = input.generics.lifetimes().next().map(|_| quote!(<'static>));
        quote! {
            const _: () = <#name #lifetime as #s::Encode>::SCHEMA_CHECKS;
            const _: () = <#name #lifetime as #s::Decode<'static>>::SCHEMA_CHECKS;
        }
    };
    Ok(quote! {
        impl #encode_impl #s::Encode for #ty #encode_where {
            const WIRE: #s::Wire = #wire;
            const FROZEN: bool = #frozen;
            const SCHEMA_CHECKS: () = { #(#frozen_checks)* };
            fn encode(&self, w: &mut #s::Writer) -> ::std::result::Result<(), #s::Error> {
                let () = <Self as #s::Encode>::SCHEMA_CHECKS;
                #encode_body
            }
            #extra_encode
        }
        impl #decode_impl #s::Decode<#de> for #ty #decode_where {
            const SCHEMA_CHECKS: () = { #(#optional_checks)* };
            fn decode(value: #s::Value<#de>) -> ::std::result::Result<Self, #s::Error> {
                let () = <Self as #s::Decode<#de>>::SCHEMA_CHECKS;
                #decode_body
            }
            #extra_decode
        }
        #forced
    })
}

fn entry<'a>(entries: &'a [(Described, Entry)], name: &str) -> &'a Entry {
    &entries
        .iter()
        .find(|(described, _)| described.name == name)
        .expect("describe gives an entry for the type and each struct variant")
        .1
}

fn entry_named<'a>(entries: &'a [(Described, Entry)], name: &str) -> &'a Entry {
    entry(entries, name)
}

/// The key a member is written with, flagged as its attributes say.
fn key(id: u32, attributes: &[syn::Attribute]) -> TokenStream {
    let critical = has_schema_word(attributes, "critical");
    let no_copy = has_schema_word(attributes, "no_copy");
    quote!(::be_wasm_schema::Key::flagged(#id, #critical, #no_copy))
}

/// The compile-time checks a type's impls force.
#[derive(Default)]
struct Checks {
    /// Asserting each type a frozen type holds is frozen.
    frozen: Vec<TokenStream>,
    /// Each field added to a frozen type after freezing, with no default of its own, and the
    /// message if it does not decode when missing.
    optional: Vec<(syn::Type, String)>,
}

impl Checks {
    fn frozen(&mut self, ty: &syn::Type, of: &str) {
        let shown = be_wasm_schema_lock::shape_of(ty);
        let message = format!(
            "{of} is frozen, so everything it holds is too, and {shown} is not: mark it #[schema(frozen)]"
        );
        self.frozen
            .push(quote!(assert!(<#ty as ::be_wasm_schema::Encode>::FROZEN, #message);));
    }
}

struct Field<'a> {
    ident: &'a Ident,
    ty: &'a syn::Type,
    id: u32,
    key: TokenStream,
    default: Option<syn::Expr>,
    slot: Ident,
    name: String,
}

/// The named fields of a struct or struct variant, by id.
struct Message<'a> {
    fields: Vec<Field<'a>>,
    unknown: Option<&'a Ident>,
    de: Lifetime,
}

impl<'a> Message<'a> {
    fn new(
        named: &'a FieldsNamed,
        entry: &Entry,
        of: &str,
        de: &Lifetime,
        frozen: bool,
        checks: &mut Checks,
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
            let default = schema_default(&field.attrs);
            if frozen {
                checks.frozen(&field.ty, &format!("{of}.{ident}"));
            }
            if default.is_none() && entry.frozen_from.is_some_and(|from| id >= from) {
                checks.optional.push((
                    field.ty.clone(),
                    format!(
                        "{of}.{ident} was added to a frozen type, so it must decode when missing: make it an Option, a collection, a number, a string or another type with a default, or give it #[schema(default = ...)]"
                    ),
                ));
            }
            fields.push(Field {
                ident,
                ty: &field.ty,
                id,
                key: key(id, &field.attrs),
                default,
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
            let write = match &field.default {
                Some(default) => quote! {
                    if !::core::cmp::PartialEq::eq(#value, &(#default)) {
                        w.field(#key, #value)?;
                    }
                },
                None => quote!(#s::Encode::encode_field(#value, #key, w)?;),
            };
            match self.unknown {
                Some(unknown) => quote! {
                    #access #unknown.write_below(#id, &mut __next, w)?;
                    #write
                },
                None => write,
            }
        });
        match self.unknown {
            Some(unknown) => quote! {
                let mut __next = 0usize;
                #(#writes)*
                #access #unknown.write_below(u32::MAX, &mut __next, w)?;
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
            match &field.default {
                Some(default) => quote! {
                    #ident: match #slot {
                        Some(value) => <#ty as #s::Decode<#de>>::decode(value)
                            .map_err(|error| error.within(#name))?,
                        None => #default,
                    },
                },
                None => quote! {
                    #ident: <#ty as #s::Decode<#de>>::decode_field(#slot)
                        .map_err(|error| error.within(#name))?,
                },
            }
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
            match &field.default {
                Some(default) => quote!(#ident: #default,),
                None => quote!(#ident: <#ty as #s::Decode<#de>>::absent()?,),
            }
        });
        let unknown = self
            .unknown
            .map(|ident| quote!(#ident: ::std::default::Default::default(),));
        quote!(Some(#path { #(#values)* #unknown }))
    }

    fn optional(&self) -> TokenStream {
        let s = quote!(::be_wasm_schema);
        let de = &self.de;
        let each = self
            .fields
            .iter()
            .filter(|field| field.default.is_none())
            .map(|field| {
                let ty = field.ty;
                quote!(<#ty as #s::Decode<#de>>::OPTIONAL)
            });
        quote!(true #(&& #each)*)
    }
}
