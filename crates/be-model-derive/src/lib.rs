use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(Model)]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unit => Vec::new(),
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(name, "a model's fields must be named")
                    .to_compile_error()
                    .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "a model is a struct")
                .to_compile_error()
                .into();
        }
    };
    if fields.len() > usize::from(u16::MAX) {
        return syn::Error::new_spanned(name, "a model has too many fields")
            .to_compile_error()
            .into();
    }

    let idents: Vec<_> = fields
        .iter()
        .map(|field| field.ident.clone().expect("named fields have names"))
        .collect();
    let types: Vec<_> = fields.iter().map(|field| field.ty.clone()).collect();
    let indices: Vec<u16> = (0..fields.len())
        .map(|index| u16::try_from(index).expect("the field count was checked"))
        .collect();
    let positions: Vec<usize> = (0..fields.len()).collect();
    let constants: Vec<_> = idents
        .iter()
        .map(|ident| format_ident!("{}", ident.to_string().to_uppercase()))
        .collect();
    let (implementation, type_arguments, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #implementation ::be_model::Model for #name #type_arguments #where_clause {
            fn blank() -> ::std::vec::Vec<::be_model::Value> {
                ::std::vec![#(<#types as ::be_model::Field>::blank()),*]
            }

            fn read(tree: &::be_model::Tree, id: ::be_model::ObjectId) -> Self {
                let fields = tree.fields::<Self>(id);
                let _ = &fields;
                Self {
                    #(#idents: <#types as ::be_model::Field>::read(tree, &fields[#positions]),)*
                }
            }

            fn write(
                &self,
                id: ::be_model::ObjectId,
                parent: ::std::option::Option<::be_model::Place>,
                out: &mut ::std::vec::Vec<(::be_model::ObjectId, ::be_model::Object)>,
            ) {
                let slot = out.len();
                out.push((id, ::be_model::Object::new(parent, ::std::vec::Vec::new())));
                let fields = ::std::vec![
                    #(<#types as ::be_model::Field>::write(&self.#idents, id, #indices, out)),*
                ];
                out[slot].1 = ::be_model::Object::new(parent, fields);
            }
        }

        impl #implementation #name #type_arguments #where_clause {
            #(
                pub const #constants: ::be_model::FieldRef<Self, #types> =
                    ::be_model::FieldRef::new(#indices);
            )*
        }
    }
    .into()
}
