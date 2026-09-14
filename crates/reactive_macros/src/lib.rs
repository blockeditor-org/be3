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
    writer: Ident,
    setter: Ident,
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
        .map(|field| {
            let name = field.ident.as_ref().expect("a named field has an ident");
            Field {
                name,
                ty: &field.ty,
                visibility: &field.vis,
                writer: format_ident!("write_{}", name),
                setter: format_ident!("set_{}", name),
            }
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
            writer,
            ..
        } = field;
        quote! {
            #visibility #name: ::reactive::ReadSignal<#ty>,
            #writer: ::reactive::WriteSignal<#ty>,
        }
    });
    let constructions = fields.iter().map(|field| {
        let Field { name, writer, .. } = field;
        quote! {
            let (#name, #writer) = ::reactive::create_signal(value.#name);
        }
    });
    let initializers = fields.iter().map(|field| {
        let Field { name, writer, .. } = field;
        quote! { #name, #writer, }
    });
    let writes = fields.iter().map(|field| {
        let Field { name, writer, .. } = field;
        quote! { self.#writer.set(value.#name); }
    });
    let reads = fields.iter().map(|field| {
        let Field { name, .. } = field;
        quote! { #name: self.#name.get(), }
    });
    let setters = fields.iter().map(|field| {
        let Field {
            ty,
            visibility,
            writer,
            setter,
            ..
        } = field;
        quote! {
            #visibility fn #setter(&self, value: #ty) {
                self.#writer.set(value);
            }
        }
    });
    let clones = fields.iter().map(|field| {
        let Field { name, writer, .. } = field;
        quote! {
            #name: self.#name.clone(),
            #writer: self.#writer.clone(),
        }
    });

    Ok(quote! {
        #visibility struct #store {
            #(#declarations)*
        }

        impl ::std::clone::Clone for #store {
            fn clone(&self) -> Self {
                Self { #(#clones)* }
            }
        }

        impl #store {
            #visibility fn new(value: #source) -> Self {
                #(#constructions)*
                Self { #(#initializers)* }
            }

            #visibility fn set(&self, value: #source) {
                ::reactive::batch(move || {
                    #(#writes)*
                });
            }

            #visibility fn get(&self) -> #source {
                #source { #(#reads)* }
            }

            #(#setters)*
        }
    })
}
