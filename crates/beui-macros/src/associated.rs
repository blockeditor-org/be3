use std::hash::{DefaultHasher, Hash, Hasher};

use proc_macro2::{Group, Punct, Spacing, TokenStream, TokenTree};
use syn::{ImplItem, ImplItemFn, ItemImpl, ItemTrait, TraitItem, TraitItemFn, parse_quote};

use super::*;

fn take_component_attr(attrs: &mut Vec<Attribute>) -> bool {
    let before = attrs.len();
    attrs.retain(|attr| {
        attr.path()
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "component")
    });
    attrs.len() != before
}

fn strip_prop_attrs(sig: &mut Signature) {
    for input in &mut sig.inputs {
        if let FnArg::Typed(typed) = input {
            typed.attrs.retain(|attr| !attr.path().is_ident("prop"));
        }
    }
}

fn reject_receiver(sig: &Signature) -> syn::Result<()> {
    match sig.receiver() {
        Some(receiver) => Err(syn::Error::new_spanned(
            receiver,
            "a `#[component]` method cannot take `self`",
        )),
        None => Ok(()),
    }
}

fn replace_self(tokens: TokenStream, with: &TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut output = TokenStream::new();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(ident) if ident == "Self" => {
                let qualified = matches!(
                    tokens.get(index + 1),
                    Some(TokenTree::Punct(punct)) if punct.as_char() == ':'
                );
                if qualified {
                    output.extend([TokenTree::Punct(Punct::new('<', Spacing::Alone))]);
                    output.extend(with.clone());
                    output.extend([TokenTree::Punct(Punct::new('>', Spacing::Alone))]);
                } else {
                    output.extend(with.clone());
                }
            }
            TokenTree::Group(group) => {
                let mut replaced =
                    Group::new(group.delimiter(), replace_self(group.stream(), with));
                replaced.set_span(group.span());
                output.extend([TokenTree::Group(replaced)]);
            }
            other => output.extend([other.clone()]),
        }
    }
    output
}

fn replace_self_in_sig(sig: &Signature, with: &TokenStream) -> syn::Result<Signature> {
    syn::parse2(replace_self(quote! { #sig }, with))
}

fn turbofish(generics: &syn::Generics) -> TokenStream {
    let args: Vec<TokenStream> = generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .map(generic_arg)
        .collect();
    if args.is_empty() {
        quote! {}
    } else {
        quote! { ::<#(#args),*> }
    }
}

fn without_default(param: &GenericParam) -> GenericParam {
    let mut param = param.clone();
    match &mut param {
        GenericParam::Type(param) => {
            param.eq_token = None;
            param.default = None;
        }
        GenericParam::Const(param) => {
            param.eq_token = None;
            param.default = None;
        }
        GenericParam::Lifetime(_) => {}
    }
    param
}

fn wrap_body(label: &str, block: &Block) -> Block {
    parse_quote! {
        {
            ::__beui_tree::reactive::component(
                &::__beui_tree::reactive::ComponentInfo { name: #label, module: module_path!() },
                move || #block,
            )
        }
    }
}

fn constructor(
    vis: &Visibility,
    sig: &Signature,
    expanded: &ExpandedComponent,
    extra_where: Option<TokenStream>,
) -> TokenStream {
    let ident = &sig.ident;
    let generics = &sig.generics;
    let predicates = generics
        .where_clause
        .iter()
        .flat_map(|clause| clause.predicates.iter())
        .map(|predicate| quote! { #predicate })
        .chain(extra_where);
    let constructor_ty = &expanded.constructor_ty;
    let constructor_body = &expanded.constructor_body;
    quote! {
        #[allow(non_snake_case)]
        #vis fn #ident #generics () -> #constructor_ty where #(#predicates,)* {
            #constructor_body
        }
    }
}

pub(crate) fn expand_trait(mut item: ItemTrait) -> syn::Result<TokenStream> {
    let trait_ident = item.ident.clone();
    let trait_args: Vec<TokenStream> = item.generics.params.iter().map(generic_arg).collect();
    let trait_args = angle_bracketed(&trait_args);
    let trait_path = quote! { #trait_ident #trait_args };
    let self_ident = quote! { __Self };
    let mut outer = vec![(parse_quote! { __Self: #trait_path }, quote! { Self })];
    outer.extend(
        item.generics
            .params
            .iter()
            .map(|param| (without_default(param), generic_arg(param))),
    );
    let outer_where: Vec<WherePredicate> = item
        .generics
        .where_clause
        .iter()
        .flat_map(|clause| clause.predicates.iter())
        .map(|predicate| syn::parse2(replace_self(quote! { #predicate }, &self_ident)))
        .collect::<syn::Result<_>>()?;

    let mut builders = TokenStream::new();
    let mut items = Vec::new();
    for trait_item in std::mem::take(&mut item.items) {
        let TraitItem::Fn(mut method) = trait_item else {
            items.push(trait_item);
            continue;
        };
        if !take_component_attr(&mut method.attrs) {
            items.push(TraitItem::Fn(method));
            continue;
        }
        reject_receiver(&method.sig)?;
        let ident = method.sig.ident.clone();
        let hidden = format_ident!("__{}", ident);
        let label = format!("{trait_ident}::{ident}");
        let method_args = turbofish(&method.sig.generics);
        let expanded = expand_builder(ComponentSpec {
            attrs: method.attrs.clone(),
            vis: item.vis.clone(),
            sig: replace_self_in_sig(&method.sig, &self_ident)?,
            label: label.clone(),
            base: format!("{trait_ident}_{ident}"),
            outer: outer.clone(),
            outer_where: outer_where.clone(),
            finish: Finish::Call(quote! { <__Self as #trait_path>::#hidden #method_args }),
        })?;
        builders.extend(expanded.items.clone());

        let mut body_sig = method.sig.clone();
        body_sig.ident = hidden;
        strip_prop_attrs(&mut body_sig);
        let mut attrs = method.attrs.clone();
        attrs.push(parse_quote! { #[doc(hidden)] });
        attrs.push(parse_quote! { #[allow(non_snake_case)] });
        items.push(TraitItem::Fn(TraitItemFn {
            attrs,
            sig: body_sig,
            default: method
                .default
                .as_ref()
                .map(|block| wrap_body(&label, block)),
            semi_token: method.semi_token,
        }));
        let constructor = constructor(
            &Visibility::Inherited,
            &method.sig,
            &expanded,
            Some(quote! { Self: ::core::marker::Sized }),
        );
        items.push(syn::parse2(constructor)?);
    }
    item.items = items;
    Ok(quote! {
        #item
        #builders
    })
}

fn type_name(ty: &Type) -> syn::Result<(String, bool)> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "`#[component]` methods go in an impl block for a named type",
        ));
    };
    let segment = path
        .path
        .segments
        .last()
        .expect("a type path has at least one segment");
    Ok((
        segment.ident.to_string(),
        !matches!(segment.arguments, PathArguments::None),
    ))
}

pub(crate) fn expand_impl(mut item: ItemImpl) -> syn::Result<TokenStream> {
    let self_ty = item.self_ty.clone();
    let (name, has_arguments) = type_name(&self_ty)?;
    let base_name = if has_arguments {
        let mut hasher = DefaultHasher::new();
        quote! { #self_ty }.to_string().hash(&mut hasher);
        format!("{name}_{:08x}", hasher.finish() as u32)
    } else {
        name.clone()
    };
    let is_trait_impl = item.trait_.is_some();
    let outer: Vec<(GenericParam, TokenStream)> = item
        .generics
        .params
        .iter()
        .map(|param| (without_default(param), generic_arg(param)))
        .collect();
    let outer_where: Vec<WherePredicate> = item
        .generics
        .where_clause
        .iter()
        .flat_map(|clause| clause.predicates.iter().cloned())
        .collect();
    let self_tokens = quote! { #self_ty };

    let mut builders = TokenStream::new();
    let mut items = Vec::new();
    for impl_item in std::mem::take(&mut item.items) {
        let ImplItem::Fn(mut method) = impl_item else {
            items.push(impl_item);
            continue;
        };
        if !take_component_attr(&mut method.attrs) {
            items.push(ImplItem::Fn(method));
            continue;
        }
        reject_receiver(&method.sig)?;
        let ident = method.sig.ident.clone();
        let hidden = format_ident!("__{}", ident);
        let label = format!("{name}::{ident}");
        let mut body_sig = method.sig.clone();
        body_sig.ident = hidden.clone();
        let mut attrs = method.attrs.clone();
        attrs.push(parse_quote! { #[doc(hidden)] });
        attrs.push(parse_quote! { #[allow(non_snake_case)] });

        if is_trait_impl {
            if let Some(attr) = method.sig.inputs.iter().find_map(|input| match input {
                FnArg::Typed(typed) => typed.attrs.iter().find(|attr| attr.path().is_ident("prop")),
                FnArg::Receiver(_) => None,
            }) {
                return Err(syn::Error::new_spanned(
                    attr,
                    format!(
                        "write `#[prop(...)]` on the trait's declaration of `{ident}`, which owns its props"
                    ),
                ));
            }
            items.push(ImplItem::Fn(ImplItemFn {
                attrs,
                vis: method.vis,
                defaultness: method.defaultness,
                sig: body_sig,
                block: wrap_body(&label, &method.block),
            }));
            continue;
        }

        let method_args = turbofish(&method.sig.generics);
        let expanded = expand_builder(ComponentSpec {
            attrs: method.attrs.clone(),
            vis: method.vis.clone(),
            sig: replace_self_in_sig(&method.sig, &self_tokens)?,
            label: label.clone(),
            base: format!("{base_name}_{ident}"),
            outer: outer.clone(),
            outer_where: outer_where.clone(),
            finish: Finish::Call(quote! { <#self_ty>::#hidden #method_args }),
        })?;
        builders.extend(expanded.items.clone());
        strip_prop_attrs(&mut body_sig);
        items.push(ImplItem::Fn(ImplItemFn {
            attrs,
            vis: Visibility::Inherited,
            defaultness: None,
            sig: body_sig,
            block: wrap_body(&label, &method.block),
        }));
        items.push(syn::parse2(constructor(
            &method.vis,
            &method.sig,
            &expanded,
            None,
        ))?);
    }
    item.items = items;
    Ok(quote! {
        #item
        #builders
    })
}
