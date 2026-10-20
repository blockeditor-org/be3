use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type, Visibility, parse_macro_input};

#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

struct Field<'a> {
    name: &'a Ident,
    ty: &'a Type,
    visibility: &'a Visibility,
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "a store cannot be derived for a generic type",
        ));
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "a store can only be derived for a struct with named fields",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "a store can only be derived for a struct with named fields",
        ));
    };
    let fields: Vec<Field<'_>> = named
        .named
        .iter()
        .map(|field| Field {
            name: field.ident.as_ref().expect("a named field has an ident"),
            ty: &field.ty,
            visibility: &field.vis,
        })
        .collect();

    let source = &input.ident;
    let store = format_ident!("{}Store", source);
    let visibility = &input.vis;

    let declarations = fields.iter().map(|field| {
        let Field {
            name,
            ty,
            visibility,
        } = field;
        quote! { #visibility #name: ::reactive::Signal<#ty>, }
    });
    let constructions = fields.iter().map(|field| {
        let name = field.name;
        quote! { #name: ::reactive::create_signal(value.#name), }
    });
    let writes = fields.iter().map(|field| {
        let name = field.name;
        quote! { ::reactive::Set::set(&self.#name, value.#name); }
    });
    let reads = fields.iter().map(|field| {
        let name = field.name;
        quote! { #name: ::reactive::Get::get(&self.#name), }
    });
    let untracked_reads: Vec<_> = fields
        .iter()
        .map(|field| {
            let name = field.name;
            quote! { #name: ::reactive::Get::get_untracked(&self.#name), }
        })
        .collect();
    Ok(quote! {
        #visibility struct #store {
            #(#declarations)*
        }

        impl ::std::clone::Clone for #store {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl ::std::marker::Copy for #store {}

        impl #store {
            #visibility fn new(value: #source) -> Self {
                Self { #(#constructions)* }
            }

            fn write(&self, value: #source) {
                ::reactive::batch(move || {
                    #(#writes)*
                });
            }
        }

        impl ::reactive::Get<#source> for #store {
            fn with<R>(&self, f: impl FnOnce(&#source) -> R) -> R {
                f(&#source { #(#reads)* })
            }

            fn with_untracked<R>(&self, f: impl FnOnce(&#source) -> R) -> R {
                f(&#source { #(#untracked_reads)* })
            }
        }

        impl ::reactive::Set<#source> for #store {
            fn update<R>(&self, f: impl FnOnce(&mut #source) -> R) -> R {
                let mut value = #source { #(#untracked_reads)* };
                let result = f(&mut value);
                self.write(value);
                result
            }

            fn set(&self, value: #source) {
                self.write(value);
            }
        }
    })
}
