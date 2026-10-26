use super::*;

pub(crate) struct Local {
    pub(crate) guest: TokenStream,
    pub(crate) embed: TokenStream,
    pub(crate) entries: Vec<TokenStream>,
}

#[derive(Clone, Copy)]
enum Flat {
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl Flat {
    fn tokens(self) -> TokenStream {
        match self {
            Self::U32 => quote!(u32),
            Self::I32 => quote!(i32),
            Self::U64 => quote!(u64),
            Self::I64 => quote!(i64),
            Self::F32 => quote!(f32),
            Self::F64 => quote!(f64),
        }
    }
}

enum Param {
    Flat,
    Bytes,
    Text,
    Words(Flat, usize),
    Handles { own: bool, resource: TypeId },
    Encoded,
}

enum Returns {
    Nothing,
    Scalar,
    Niche { own: bool },
    Encoded,
}

struct Call {
    key: String,
    host: Ident,
    guest: Ident,
    import: Ident,
    resource: Option<TypeId>,
    receiver: bool,
    constructor: bool,
    params: Vec<(Ident, Type)>,
    result: Option<Type>,
}

impl Generator {
    fn handle_of(&self, ty: &Type) -> Option<(bool, TypeId)> {
        let Type::Id(id) = ty else {
            return None;
        };
        match &self.resolve.types[self.target(*id)].kind {
            TypeDefKind::Handle(wit_parser::Handle::Own(resource)) => {
                Some((true, self.target(*resource)))
            }
            TypeDefKind::Handle(wit_parser::Handle::Borrow(resource)) => {
                Some((false, self.target(*resource)))
            }
            _ => None,
        }
    }

    fn optional_handle(&self, ty: &Type) -> Option<(bool, TypeId)> {
        let Type::Id(id) = ty else {
            return None;
        };
        match &self.resolve.types[self.target(*id)].kind {
            TypeDefKind::Option(inner) => self.handle_of(inner),
            _ => None,
        }
    }

    fn flat(&self, ty: &Type) -> Option<Vec<Flat>> {
        Some(match ty {
            Type::Bool | Type::U8 | Type::U16 | Type::U32 | Type::Char => vec![Flat::U32],
            Type::S8 | Type::S16 | Type::S32 => vec![Flat::I32],
            Type::U64 => vec![Flat::U64],
            Type::S64 => vec![Flat::I64],
            Type::F32 => vec![Flat::F32],
            Type::F64 => vec![Flat::F64],
            Type::String | Type::ErrorContext => return None,
            Type::Id(id) => {
                let id = self.target(*id);
                if self.mapped(id).is_some() {
                    return None;
                }
                match &self.resolve.types[id].kind {
                    TypeDefKind::Record(record) => {
                        let mut flat = Vec::new();
                        for field in &record.fields {
                            flat.extend(self.flat(&field.ty)?);
                        }
                        flat
                    }
                    TypeDefKind::Tuple(tuple) => {
                        let mut flat = Vec::new();
                        for ty in &tuple.types {
                            flat.extend(self.flat(ty)?);
                        }
                        flat
                    }
                    TypeDefKind::Enum(_) | TypeDefKind::Flags(_) | TypeDefKind::Handle(_) => {
                        vec![Flat::U32]
                    }
                    TypeDefKind::Option(inner) => {
                        let mut flat = vec![Flat::U32];
                        flat.extend(self.flat(inner)?);
                        flat
                    }
                    TypeDefKind::Type(inner) => self.flat(inner)?,
                    _ => return None,
                }
            }
        })
    }

    fn lower(&self, ty: &Type, value: TokenStream, depth: usize) -> Vec<TokenStream> {
        match ty {
            Type::Bool | Type::U8 | Type::U16 | Type::Char => vec![quote!((#value) as u32)],
            Type::S8 | Type::S16 => vec![quote!((#value) as i32)],
            Type::U32 | Type::S32 | Type::U64 | Type::S64 | Type::F32 | Type::F64 => {
                vec![value]
            }
            Type::String | Type::ErrorContext => Vec::new(),
            Type::Id(id) => {
                let id = self.target(*id);
                match &self.resolve.types[id].kind {
                    TypeDefKind::Record(record) => record
                        .fields
                        .iter()
                        .flat_map(|field| {
                            let name = snake(&field.name);
                            self.lower(&field.ty, quote!((#value).#name), depth)
                        })
                        .collect(),
                    TypeDefKind::Tuple(tuple) => tuple
                        .types
                        .iter()
                        .enumerate()
                        .flat_map(|(index, ty)| {
                            let index = syn::Index::from(index);
                            self.lower(ty, quote!((#value).#index), depth)
                        })
                        .collect(),
                    TypeDefKind::Enum(_) => vec![quote!((#value) as u32)],
                    TypeDefKind::Flags(_) => vec![quote!((#value).bits())],
                    TypeDefKind::Handle(_) => vec![quote!((#value).0)],
                    TypeDefKind::Option(inner) => {
                        let bound = format_ident!("inner{depth}");
                        let mut lowered = vec![quote! {
                            match #value {
                                ::core::option::Option::Some(_) => 1u32,
                                ::core::option::Option::None => 0u32,
                            }
                        }];
                        let flat = self.flat(inner).unwrap_or_default();
                        let inner = self.lower(inner, quote!(#bound), depth + 1);
                        for (lowered_inner, flat) in inner.into_iter().zip(flat) {
                            let ty = flat.tokens();
                            lowered.push(quote! {
                                match #value {
                                    ::core::option::Option::Some(#bound) => #lowered_inner,
                                    ::core::option::Option::None => {
                                        <#ty as ::core::default::Default>::default()
                                    }
                                }
                            });
                        }
                        lowered
                    }
                    TypeDefKind::Type(inner) => self.lower(inner, value, depth),
                    _ => Vec::new(),
                }
            }
        }
    }

    fn lift(&self, ty: &Type, values: &mut std::slice::Iter<'_, Ident>) -> TokenStream {
        let mut next = || {
            let value = values.next();
            quote!(#value)
        };
        match ty {
            Type::Bool => {
                let value = next();
                quote!(#value != 0)
            }
            Type::U8 => {
                let value = next();
                quote!(#value as u8)
            }
            Type::U16 => {
                let value = next();
                quote!(#value as u16)
            }
            Type::S8 => {
                let value = next();
                quote!(#value as i8)
            }
            Type::S16 => {
                let value = next();
                quote!(#value as i16)
            }
            Type::Char => {
                let value = next();
                quote! {
                    ::core::char::from_u32(#value)
                        .ok_or_else(|| ::std::format!("{} is not a char", #value))?
                }
            }
            Type::U32 | Type::S32 | Type::U64 | Type::S64 | Type::F32 | Type::F64 => next(),
            Type::String | Type::ErrorContext => TokenStream::new(),
            Type::Id(id) => {
                let id = self.target(*id);
                let path = self.ty(&Type::Id(id)).unwrap_or_default();
                match &self.resolve.types[id].kind {
                    TypeDefKind::Record(record) => {
                        let fields = record.fields.iter().map(|field| {
                            let name = snake(&field.name);
                            let value = self.lift(&field.ty, values);
                            quote!(#name: #value)
                        });
                        let fields: Vec<_> = fields.collect();
                        quote!(#path { #(#fields),* })
                    }
                    TypeDefKind::Tuple(tuple) => {
                        let items: Vec<_> =
                            tuple.types.iter().map(|ty| self.lift(ty, values)).collect();
                        quote!((#(#items,)*))
                    }
                    TypeDefKind::Enum(cases) => {
                        let value = next();
                        let name = self.type_name(id).unwrap_or_default();
                        let arms = cases.cases.iter().enumerate().map(|(index, case)| {
                            let index = index as u32;
                            let case = camel(&case.name);
                            quote!(#index => #path::#case,)
                        });
                        quote! {
                            match #value {
                                #(#arms)*
                                other => {
                                    return ::core::result::Result::Err(
                                        ::std::format!("{other} is not a {}", #name),
                                    );
                                }
                            }
                        }
                    }
                    TypeDefKind::Flags(_) => {
                        let value = next();
                        quote!(#path::from_bits(#value))
                    }
                    TypeDefKind::Handle(_) => {
                        let value = next();
                        quote!(#path(#value))
                    }
                    TypeDefKind::Option(inner) => {
                        let tag = next();
                        let inner = self.lift(inner, values);
                        quote! {
                            match #tag {
                                0 => ::core::option::Option::None,
                                _ => ::core::option::Option::Some(#inner),
                            }
                        }
                    }
                    TypeDefKind::Type(inner) => self.lift(inner, values),
                    _ => TokenStream::new(),
                }
            }
        }
    }

    fn param(&self, ty: &Type) -> Param {
        if let Type::String = ty {
            return Param::Text;
        }
        if let Type::Id(id) = ty {
            let id = self.target(*id);
            if let TypeDefKind::List(inner) = &self.resolve.types[id].kind
                && self.mapped(id).is_none()
            {
                if let Some((own, resource)) = self.handle_of(inner) {
                    return Param::Handles { own, resource };
                }
                return match inner {
                    Type::U8 => Param::Bytes,
                    Type::U32 => Param::Words(Flat::U32, 4),
                    Type::S32 => Param::Words(Flat::I32, 4),
                    Type::U64 => Param::Words(Flat::U64, 8),
                    Type::S64 => Param::Words(Flat::I64, 8),
                    Type::F32 => Param::Words(Flat::F32, 4),
                    Type::F64 => Param::Words(Flat::F64, 8),
                    _ => Param::Encoded,
                };
            }
        }
        match self.flat(ty) {
            Some(_) => Param::Flat,
            None => Param::Encoded,
        }
    }

    fn returns(&self, ty: Option<&Type>) -> Returns {
        let Some(ty) = ty else {
            return Returns::Nothing;
        };
        if let Some((own, _)) = self.optional_handle(ty) {
            return Returns::Niche { own };
        }
        match self.flat(ty) {
            Some(flat) if flat.len() == 1 => Returns::Scalar,
            _ => Returns::Encoded,
        }
    }

    fn resource_names(&self, resource: TypeId) -> (Ident, String) {
        let name = self.resolve.types[resource]
            .name
            .clone()
            .unwrap_or_default();
        (camel(&name), name)
    }

    fn owner(&self, resource: TypeId) -> TokenStream {
        let (name, _) = self.resource_names(resource);
        let TypeOwner::Interface(interface) = self.resolve.types[resource].owner else {
            return quote!(#name);
        };
        let module = self.module_name(interface);
        quote!(crate::guest::#module::#name)
    }

    fn handle_value(&self, resource: TypeId) -> TokenStream {
        let (name, _) = self.resource_names(resource);
        let TypeOwner::Interface(interface) = self.resolve.types[resource].owner else {
            return quote!(#name);
        };
        let module = self.module_name(interface);
        quote!(crate::#module::#name)
    }

    fn abi(&self, ty: &Type) -> Vec<Flat> {
        match self.param(ty) {
            Param::Flat => self.flat(ty).unwrap_or_default(),
            _ => vec![Flat::U32, Flat::U32],
        }
    }

    fn calls(&self, interface: InterfaceId) -> Result<Vec<Call>, String> {
        let mut calls = Vec::new();
        for (key, function) in &self.resolve.interfaces[interface].functions {
            if function.kind.is_async() {
                return Err(format!(
                    "{key}: a local interface is answered at once, so it has no async funcs"
                ));
            }
            let item = function.item_name();
            let (resource, receiver, constructor, host) = match function.kind {
                FunctionKind::Freestanding => (None, false, false, item.to_owned()),
                FunctionKind::Method(resource) => {
                    let resource = self.target(resource);
                    let (_, name) = self.resource_names(resource);
                    (Some(resource), true, false, format!("{name}-{item}"))
                }
                FunctionKind::Constructor(resource) => {
                    let resource = self.target(resource);
                    let (_, name) = self.resource_names(resource);
                    (Some(resource), false, true, format!("{name}-new"))
                }
                _ => return Err(format!("{key}: static functions are not supported")),
            };
            let params = function
                .params
                .iter()
                .skip(receiver as usize)
                .map(|param| (snake(&param.name), param.ty))
                .collect();
            let result = match constructor {
                true => None,
                false => function.result,
            };
            if let Some(ty) = &result
                && let Type::Id(id) = ty
                && let TypeDefKind::Stream(_) | TypeDefKind::Future(_) =
                    self.resolve.types[*id].kind
            {
                return Err(format!("{key}: a local func returns no stream or future"));
            }
            calls.push(Call {
                key: key.clone(),
                host: snake(&host),
                guest: match constructor {
                    true => snake("new"),
                    false => snake(item),
                },
                import: snake(&host),
                resource,
                receiver,
                constructor,
                params,
                result,
            });
        }
        for (id, _) in self.resources(interface) {
            let (_, name) = self.resource_names(id);
            calls.push(Call {
                key: format!("[resource-drop]{name}"),
                host: snake(&format!("{name}-drop")),
                guest: snake("drop"),
                import: snake(&format!("{name}-drop")),
                resource: Some(id),
                receiver: true,
                constructor: false,
                params: Vec::new(),
                result: None,
            });
        }
        Ok(calls)
    }

    pub(crate) fn local(&self, interface: InterfaceId) -> Result<Local, String> {
        let id = self
            .resolve
            .id_of(interface)
            .ok_or("an interface of the world has no name")?;
        let module = self.module_name(interface);
        let calls = self.calls(interface)?;
        let encoded_results = calls
            .iter()
            .any(|call| matches!(self.returns(call.result.as_ref()), Returns::Encoded));
        let mut externs = Vec::new();
        let mut stubs = Vec::new();
        let mut free = Vec::new();
        let mut methods = BTreeMap::<TypeId, Vec<TokenStream>>::new();
        let mut drops = BTreeMap::<TypeId, TokenStream>::new();
        let mut host_items = Vec::new();
        let mut glue = Vec::new();
        let mut entries = Vec::new();
        let dollar = proc_macro2::Punct::new('$', proc_macro2::Spacing::Alone);
        for call in &calls {
            for (_, ty) in &call.params {
                self.ty(ty)?;
            }
            if let Some(ty) = &call.result {
                self.ty(ty)?;
            }
            let returns = self.returns(call.result.as_ref());
            let mut abi_types: Vec<Flat> = Vec::new();
            if call.receiver {
                abi_types.push(Flat::U32);
            }
            for (_, ty) in &call.params {
                abi_types.extend(self.abi(ty));
            }
            if let Returns::Encoded = returns {
                abi_types.extend([Flat::U32, Flat::U32]);
            }
            if abi_types.len() > 16 {
                return Err(format!(
                    "{}: {} wasm arguments is more than the 16 a call may take",
                    call.key,
                    abi_types.len()
                ));
            }
            let abi_result = match (&returns, call.constructor) {
                (_, true) => Some(Flat::U32),
                (Returns::Nothing, _) => None,
                (Returns::Scalar, _) => call
                    .result
                    .as_ref()
                    .and_then(|ty| self.flat(ty))
                    .and_then(|flat| flat.first().copied()),
                (Returns::Niche { .. } | Returns::Encoded, _) => Some(Flat::U32),
            };
            let abi_names: Vec<Ident> = (0..abi_types.len())
                .map(|index| format_ident!("a{index}"))
                .collect();
            let abi_params: Vec<TokenStream> = abi_names
                .iter()
                .zip(&abi_types)
                .map(|(name, ty)| {
                    let ty = ty.tokens();
                    quote!(#name: #ty)
                })
                .collect();
            let abi_return = abi_result.map(|ty| {
                let ty = ty.tokens();
                quote!(-> #ty)
            });
            let key = &call.key;
            let import = &call.import;
            externs.push(quote! {
                #[link_name = #key]
                pub fn #import(#(#abi_params),*) #abi_return;
            });
            stubs.push(quote! {
                pub unsafe fn #import(#(#abi_params),*) #abi_return {
                    let _ = (#(#abi_names,)*);
                    ::core::unreachable!("{} is only callable from wasm", #key)
                }
            });

            let mut guest_params = Vec::new();
            let mut guest_prepare = Vec::new();
            let mut guest_args = Vec::new();
            if call.receiver {
                guest_args.push(quote!((self.0).0));
            }
            for (name, ty) in &call.params {
                match self.param(ty) {
                    Param::Flat => {
                        if let Some((own, resource)) = self.handle_of(ty) {
                            let owner = self.owner(resource);
                            guest_params.push(match own {
                                true => quote!(#name: #owner),
                                false => quote!(#name: &#owner),
                            });
                            guest_prepare.push(match own {
                                true => quote!(let #name = #name.into_handle();),
                                false => quote!(let #name = #name.handle();),
                            });
                        } else if let Some((own, resource)) = self.optional_handle(ty) {
                            let owner = self.owner(resource);
                            guest_params.push(match own {
                                true => quote!(#name: ::core::option::Option<#owner>),
                                false => quote!(#name: ::core::option::Option<&#owner>),
                            });
                            guest_prepare.push(match own {
                                true => quote!(let #name = #name.map(|#name| #name.into_handle());),
                                false => quote!(let #name = #name.map(|#name| #name.handle());),
                            });
                        } else {
                            let rust = self.ty(ty)?;
                            guest_params.push(quote!(#name: #rust));
                        }
                        guest_args.extend(self.lower(ty, quote!(#name), 0));
                    }
                    Param::Bytes => {
                        guest_params.push(quote!(#name: &[u8]));
                        guest_args.push(quote!(#name.as_ptr() as u32));
                        guest_args.push(quote!(#name.len() as u32));
                    }
                    Param::Text => {
                        guest_params.push(quote!(#name: &str));
                        guest_args.push(quote!(#name.as_ptr() as u32));
                        guest_args.push(quote!(#name.len() as u32));
                    }
                    Param::Words(flat, _) => {
                        let ty = flat.tokens();
                        guest_params.push(quote!(#name: &[#ty]));
                        guest_args.push(quote!(#name.as_ptr() as u32));
                        guest_args.push(quote!(#name.len() as u32));
                    }
                    Param::Handles { own, resource } => {
                        let owner = self.owner(resource);
                        guest_params.push(match own {
                            true => quote!(#name: ::std::vec::Vec<#owner>),
                            false => quote!(#name: &[&#owner]),
                        });
                        guest_prepare.push(match own {
                            true => quote! {
                                let #name: ::std::vec::Vec<u32> = #name
                                    .into_iter()
                                    .map(|#name| #name.into_handle().0)
                                    .collect();
                            },
                            false => quote! {
                                let #name: ::std::vec::Vec<u32> =
                                    #name.iter().map(|#name| #name.handle().0).collect();
                            },
                        });
                        guest_args.push(quote!(#name.as_ptr() as u32));
                        guest_args.push(quote!(#name.len() as u32));
                    }
                    Param::Encoded => {
                        let rust = self.ty(ty)?;
                        guest_params.push(quote!(#name: &#rust));
                        guest_prepare.push(quote!(let #name = ::be_ops::encode(#name);));
                        guest_args.push(quote!(#name.as_ptr() as u32));
                        guest_args.push(quote!(#name.len() as u32));
                    }
                }
            }
            let invoke = quote!(unsafe { imports::#import(#(#guest_args),*) });
            let (guest_return, guest_body) = match (&returns, call.constructor) {
                (_, true) => {
                    let value = self.handle_value(resource_of(call)?);
                    (quote!(-> Self), quote!(Self(#value(#invoke))))
                }
                (Returns::Nothing, _) => (TokenStream::new(), quote!(#invoke;)),
                (Returns::Scalar, _) => {
                    let ty = call.result.unwrap_or(Type::Bool);
                    match self.handle_of(&ty) {
                        Some((true, resource)) => {
                            let owner = self.owner(resource);
                            let value = self.handle_value(resource);
                            (quote!(-> #owner), quote!(#owner(#value(#invoke))))
                        }
                        _ => {
                            let rust = self.ty(&ty)?;
                            let value = format_ident!("returned");
                            let lifted = self.lift(&ty, &mut [value.clone()].iter());
                            (
                                quote!(-> #rust),
                                quote! {
                                    let #value = #invoke;
                                    let lifted = (|| -> ::core::result::Result<#rust, ::std::string::String> {
                                        ::core::result::Result::Ok(#lifted)
                                    })();
                                    match lifted {
                                        ::core::result::Result::Ok(value) => value,
                                        ::core::result::Result::Err(error) => {
                                            ::core::panic!("{} returned what it cannot: {error}", #key)
                                        }
                                    }
                                },
                            )
                        }
                    }
                }
                (Returns::Niche { own }, _) => {
                    let (_, resource) = call
                        .result
                        .as_ref()
                        .and_then(|ty| self.optional_handle(ty))
                        .ok_or_else(|| format!("{}: no handle is returned", call.key))?;
                    let value = self.handle_value(resource);
                    match own {
                        true => {
                            let owner = self.owner(resource);
                            (
                                quote!(-> ::core::option::Option<#owner>),
                                quote! {
                                    match #invoke {
                                        0 => ::core::option::Option::None,
                                        handle => ::core::option::Option::Some(#owner(#value(handle))),
                                    }
                                },
                            )
                        }
                        false => (
                            quote!(-> ::core::option::Option<#value>),
                            quote! {
                                match #invoke {
                                    0 => ::core::option::Option::None,
                                    handle => ::core::option::Option::Some(#value(handle)),
                                }
                            },
                        ),
                    }
                }
                (Returns::Encoded, _) => {
                    let rust = self.ty(call.result.as_ref().unwrap_or(&Type::Bool))?;
                    let mut args = guest_args.clone();
                    args.push(quote!(pointer));
                    args.push(quote!(capacity));
                    (
                        quote!(-> #rust),
                        quote! {
                            returned(#key, |pointer, capacity| unsafe { imports::#import(#(#args),*) })
                        },
                    )
                }
            };
            let guest_name = &call.guest;
            match (call.resource, call.receiver, call.constructor) {
                (Some(resource), true, _) if key.starts_with("[resource-drop]") => {
                    drops.insert(resource, quote! { #invoke; });
                }
                (Some(resource), true, _) => {
                    methods.entry(resource).or_default().push(quote! {
                        #[inline]
                        pub fn #guest_name(&self, #(#guest_params),*) #guest_return {
                            #(#guest_prepare)*
                            #guest_body
                        }
                    });
                }
                (Some(resource), _, true) => {
                    methods.entry(resource).or_default().push(quote! {
                        #[inline]
                        pub fn #guest_name(#(#guest_params),*) #guest_return {
                            #(#guest_prepare)*
                            #guest_body
                        }
                    });
                }
                _ => free.push(quote! {
                    #[inline]
                    pub fn #guest_name(#(#guest_params),*) #guest_return {
                        #(#guest_prepare)*
                        #guest_body
                    }
                }),
            }

            let mut host_params = Vec::new();
            let mut lifts = Vec::new();
            let mut passed = Vec::new();
            let mut values = abi_names.iter();
            if call.receiver
                && let Some(resource) = call.resource
            {
                let value = self.handle_value(resource);
                host_params.push(quote!(this: #value));
                let name = values.next();
                lifts.push(quote!(#value(#name)));
                passed.push(quote!(this));
            }
            let mut bound = Vec::new();
            if call.receiver {
                bound.push(format_ident!("this"));
            }
            for (name, ty) in &call.params {
                bound.push(name.clone());
                match self.param(ty) {
                    Param::Flat => {
                        let rust = self.ty(ty)?;
                        host_params.push(quote!(#name: #rust));
                        lifts.push(self.lift(ty, &mut values));
                        passed.push(quote!(#name));
                    }
                    kind => {
                        let (Some(pointer), Some(length)) = (values.next(), values.next()) else {
                            continue;
                        };
                        let read = quote!(caller.read(#pointer, #length)?);
                        let (host_type, lift, pass) = match kind {
                            Param::Bytes => (quote!(&[u8]), read, quote!(&#name)),
                            Param::Text => (
                                quote!(&str),
                                quote! {
                                    ::std::string::String::from_utf8(#read)
                                        .map_err(|_| "a string is not utf-8".to_owned())?
                                },
                                quote!(&#name),
                            ),
                            Param::Words(flat, size) => {
                                let ty = flat.tokens();
                                (
                                    quote!(&[#ty]),
                                    words(pointer, length, size, quote!(#ty::from_le_bytes)),
                                    quote!(&#name),
                                )
                            }
                            Param::Handles { resource, .. } => {
                                let value = self.handle_value(resource);
                                (
                                    quote!(&[#value]),
                                    words(
                                        pointer,
                                        length,
                                        4,
                                        quote!(|bytes| #value(u32::from_le_bytes(bytes))),
                                    ),
                                    quote!(&#name),
                                )
                            }
                            _ => {
                                let rust = self.ty(ty)?;
                                (
                                    rust.clone(),
                                    quote! {
                                        ::be_ops::decode::<#rust>(&#read).ok_or_else(|| {
                                            ::std::format!("{} does not decode", ::core::stringify!(#name))
                                        })?
                                    },
                                    quote!(#name),
                                )
                            }
                        };
                        host_params.push(quote!(#name: #host_type));
                        lifts.push(lift);
                        passed.push(pass);
                    }
                }
            }
            let host_name = &call.host;
            let (host_return, lowered) = match (&returns, call.constructor) {
                (_, true) => {
                    let value = self.handle_value(resource_of(call)?);
                    (quote!(-> #value), quote!(result.0))
                }
                (Returns::Nothing, _) => (TokenStream::new(), quote!(result)),
                (Returns::Scalar, _) => {
                    let ty = call.result.unwrap_or(Type::Bool);
                    let rust = self.ty(&ty)?;
                    let lowered = self.lower(&ty, quote!(result), 0);
                    (quote!(-> #rust), quote!(#(#lowered)*))
                }
                (Returns::Niche { .. }, _) => {
                    let rust = self.ty(call.result.as_ref().unwrap_or(&Type::Bool))?;
                    (
                        quote!(-> #rust),
                        quote! {
                            match result {
                                ::core::option::Option::Some(handle) => handle.0,
                                ::core::option::Option::None => 0,
                            }
                        },
                    )
                }
                (Returns::Encoded, _) => {
                    let rust = self.ty(call.result.as_ref().unwrap_or(&Type::Bool))?;
                    let pointer = &abi_names[abi_names.len() - 2];
                    let capacity = &abi_names[abi_names.len() - 1];
                    (
                        quote!(-> #rust),
                        quote! {{
                            let bytes = ::be_ops::encode(&result);
                            let length = bytes.len() as u32;
                            if length <= #capacity {
                                if let ::core::result::Result::Err(message) = caller.write(#pointer, &bytes) {
                                    caller.refuse(::std::format!("{}: {message}", #key));
                                    return 0;
                                }
                            } else {
                                *caller.returned() = bytes;
                            }
                            length
                        }},
                    )
                }
            };
            host_items.push(quote! {
                fn #host_name(&mut self, #(#host_params),*) #host_return;
            });
            let glue_return = abi_return.clone().unwrap_or_default();
            glue.push(quote! {
                #[inline]
                pub fn #host_name<C: super::Caller>(caller: &mut C, #(#abi_params),*) #glue_return {
                    let lifted = (|| -> ::core::result::Result<_, ::std::string::String> {
                        ::core::result::Result::Ok((#(#lifts,)*))
                    })();
                    let (#(#bound,)*) = match lifted {
                        ::core::result::Result::Ok(lifted) => lifted,
                        ::core::result::Result::Err(message) => {
                            caller.refuse(::std::format!("{}: {message}", #key));
                            return ::core::default::Default::default();
                        }
                    };
                    let ::core::option::Option::Some(result) =
                        caller.host(#key, |host| host.#host_name(#(#passed),*))
                    else {
                        return ::core::default::Default::default();
                    };
                    #lowered
                }
            });
            let export = format!("{id}#{key}");
            let entry_params: Vec<TokenStream> = abi_params.clone();
            entries.push(quote! {
                #dollar crate::embed::#module::glue::#host_name,
                #id, #key, #export, (#(#entry_params),*) #abi_return;
            });
        }
        if encoded_results {
            externs.push(quote! {
                #[link_name = "[returned]"]
                pub fn returned(pointer: u32);
            });
            stubs.push(quote! {
                pub unsafe fn returned(pointer: u32) {
                    let _ = pointer;
                    ::core::unreachable!("[returned] is only callable from wasm")
                }
            });
            glue.push(quote! {
                pub fn returned<C: super::Caller>(caller: &mut C, pointer: u32) {
                    let bytes = ::core::mem::take(caller.returned());
                    if let ::core::result::Result::Err(message) = caller.write(pointer, &bytes) {
                        caller.refuse(::std::format!("[returned]: {message}"));
                    }
                }
            });
            let export = format!("{id}#[returned]");
            entries.push(quote! {
                #dollar crate::embed::#module::glue::returned,
                #id, "[returned]", #export, (a0: u32);
            });
        }
        let returned_helper = match encoded_results {
            true => quote! {
                ::std::thread_local! {
                    static RETURNED: ::core::cell::RefCell<::std::vec::Vec<u8>> =
                        const { ::core::cell::RefCell::new(::std::vec::Vec::new()) };
                }

                fn returned<T: ::serde::de::DeserializeOwned>(
                    name: &str,
                    call: impl FnOnce(u32, u32) -> u32,
                ) -> T {
                    RETURNED.with(|room| {
                        let mut room = room.borrow_mut();
                        if room.len() < 256 {
                            room.resize(256, 0);
                        }
                        let length = call(room.as_mut_ptr() as u32, room.len() as u32) as usize;
                        if length > room.len() {
                            room.resize(length, 0);
                            unsafe { imports::returned(room.as_mut_ptr() as u32) };
                        }
                        match ::be_ops::decode(&room[..length]) {
                            ::core::option::Option::Some(value) => value,
                            ::core::option::Option::None => {
                                ::core::panic!("{name} returned what does not decode")
                            }
                        }
                    })
                }
            },
            false => TokenStream::new(),
        };
        let owners = self.resources(interface).into_iter().map(|(resource, _)| {
            let (name, _) = self.resource_names(resource);
            let value = self.handle_value(resource);
            let items = methods.remove(&resource).unwrap_or_default();
            let drop = drops.remove(&resource).unwrap_or_default();
            quote! {
                #[derive(Debug, PartialEq, Eq)]
                pub struct #name(#value);

                impl #name {
                    pub fn from_handle(handle: #value) -> Self {
                        Self(handle)
                    }

                    pub fn handle(&self) -> #value {
                        self.0
                    }

                    pub fn into_handle(self) -> #value {
                        let handle = self.0;
                        ::core::mem::forget(self);
                        handle
                    }

                    #(#items)*
                }

                impl ::core::ops::Drop for #name {
                    fn drop(&mut self) {
                        #drop
                    }
                }
            }
        });
        let guest = quote! {
            mod imports {
                #[cfg(target_arch = "wasm32")]
                #[link(wasm_import_module = #id)]
                unsafe extern "C" {
                    #(#externs)*
                }

                #[cfg(not(target_arch = "wasm32"))]
                mod absent {
                    #(#stubs)*
                }

                #[cfg(not(target_arch = "wasm32"))]
                pub use absent::*;
            }

            #returned_helper

            #(#owners)*

            #(#free)*
        };
        let embed = quote! {
            pub const MODULE: &str = #id;

            pub trait Host {
                #(#host_items)*
            }

            pub trait Caller {
                type Host: Host;

                fn read(&mut self, pointer: u32, length: u32)
                    -> ::core::result::Result<::std::vec::Vec<u8>, ::std::string::String>;

                fn write(&mut self, pointer: u32, bytes: &[u8])
                    -> ::core::result::Result<(), ::std::string::String>;

                fn host<R>(
                    &mut self,
                    function: &'static str,
                    call: impl FnOnce(&mut Self::Host) -> R,
                ) -> ::core::option::Option<R>;

                fn refuse(&mut self, message: ::std::string::String);

                fn returned(&mut self) -> &mut ::std::vec::Vec<u8>;
            }

            pub mod glue {
                use super::Host as _;

                #(#glue)*
            }
        };
        Ok(Local {
            guest,
            embed,
            entries,
        })
    }
}

fn resource_of(call: &Call) -> Result<TypeId, String> {
    call.resource
        .ok_or_else(|| format!("{}: a constructor without its resource", call.key))
}

fn words(pointer: &Ident, length: &Ident, size: usize, make: TokenStream) -> TokenStream {
    quote! {{
        let length = #length
            .checked_mul(#size as u32)
            .ok_or_else(|| "a list is too long".to_owned())?;
        let bytes = caller.read(#pointer, length)?;
        let mut words = ::std::vec::Vec::with_capacity(bytes.len() / #size);
        for chunk in bytes.chunks_exact(#size) {
            let mut word = [0u8; #size];
            word.copy_from_slice(chunk);
            words.push((#make)(word));
        }
        words
    }}
}
