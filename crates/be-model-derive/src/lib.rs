use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitInt, LitStr, Path, parse_macro_input};

#[derive(Default)]
struct KindAttributes {
    kind: Option<LitStr>,
    format: Option<LitInt>,
    migrate: Option<Path>,
}

#[derive(Default)]
struct FieldAttributes {
    rename: Option<LitStr>,
    aliases: Vec<LitStr>,
    from_old: Vec<(LitStr, Path)>,
    critical: bool,
}

fn kind_attributes(input: &DeriveInput) -> syn::Result<KindAttributes> {
    let mut out = KindAttributes::default();
    for attribute in &input.attrs {
        if !attribute.path().is_ident("model") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("kind") {
                out.kind = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("format") {
                out.format = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("migrate") {
                out.migrate = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("expected kind, format or migrate"));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn field_attributes(field: &syn::Field) -> syn::Result<FieldAttributes> {
    let mut out = FieldAttributes::default();
    for attribute in &field.attrs {
        if attribute.path().is_ident("critical") {
            out.critical = true;
            continue;
        }
        if !attribute.path().is_ident("model") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                out.rename = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("alias") {
                out.aliases.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("from_old") {
                let content;
                syn::parenthesized!(content in meta.input);
                let name: LitStr = content.parse()?;
                content.parse::<syn::Token![,]>()?;
                let convert: Path = content.parse()?;
                out.from_old.push((name, convert));
            } else {
                return Err(meta.error("expected rename, alias or from_old"));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[proc_macro_derive(Model, attributes(model, critical))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn derive(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unit => Vec::new(),
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    name,
                    "a model's fields must be named",
                ));
            }
        },
        _ => return Err(syn::Error::new_spanned(name, "a model is a struct")),
    };
    if fields.len() > usize::from(u16::MAX) {
        return Err(syn::Error::new_spanned(name, "a model has too many fields"));
    }
    let kind = kind_attributes(input)?;
    let kind_name = kind
        .kind
        .map_or_else(|| snake_case(&name.to_string()), |kind| kind.value());
    let format = kind
        .format
        .map_or_else(|| quote!(1), |format| quote!(#format));
    let migrate = kind.migrate.map_or_else(
        || quote!(::be_model::schema::no_migration),
        |migrate| quote!(#migrate),
    );

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
    let mut properties = Vec::new();
    for ((field, ident), ty) in fields.iter().zip(&idents).zip(&types) {
        let attributes = field_attributes(field)?;
        let stored_name = attributes
            .rename
            .map_or_else(|| ident.to_string(), |rename| rename.value());
        let aliases = &attributes.aliases;
        let critical = attributes.critical;
        let old_names: Vec<_> = attributes.from_old.iter().map(|(name, _)| name).collect();
        let converts: Vec<_> = attributes.from_old.iter().map(|(_, path)| path).collect();
        properties.push(quote! {
            ::be_model::Property {
                name: #stored_name,
                aliases: ::std::vec![#(#aliases),*],
                from_old: ::std::vec![#((
                    #old_names,
                    (|old: &::be_model::Stored| ::be_model::schema::convert(old, #converts))
                        as ::be_model::schema::Convert,
                )),*],
                critical: #critical,
                shape: <#ty as ::be_model::Field>::shape(),
            }
        });
    }
    let (implementation, type_arguments, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
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

            fn kind() -> ::be_model::Kind {
                ::be_model::Kind {
                    name: #kind_name,
                    format: #format,
                    migrate: #migrate,
                    blank: <Self as ::be_model::Model>::blank,
                    properties: ::std::vec![#(#properties),*],
                }
            }
        }

        impl #implementation #name #type_arguments #where_clause {
            #(
                pub const #constants: ::be_model::FieldRef<Self, #types> =
                    ::be_model::FieldRef::new(#indices);
            )*
        }
    })
}
