use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use heck::{ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Ident, LitStr, Token, braced, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};
use wit_parser::{
    Function, FunctionKind, InterfaceId, Resolve, Type, TypeDefKind, TypeId, TypeOwner, WorldItem,
};

#[derive(Default)]
struct Options {
    path: String,
    world: String,
    with: BTreeMap<String, syn::Type>,
    encodings: BTreeMap<String, syn::Type>,
    uses: BTreeMap<String, syn::Path>,
    own_debug: BTreeSet<String>,
    copy: BTreeSet<String>,
    default: BTreeSet<String>,
    inline: BTreeSet<String>,
    lenient: BTreeSet<String>,
    ordered: BTreeSet<String>,
    deps: Vec<String>,
    local: BTreeSet<String>,
    network: bool,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);
        let mut options = Options::default();
        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "path" => options.path = content.parse::<LitStr>()?.value(),
                "world" => options.world = content.parse::<LitStr>()?.value(),
                "network" => options.network = content.parse::<syn::LitBool>()?.value,
                "with" | "encodings" => {
                    let target = match key.to_string().as_str() {
                        "with" => &mut options.with,
                        _ => &mut options.encodings,
                    };
                    let mapped;
                    braced!(mapped in content);
                    while !mapped.is_empty() {
                        let name: LitStr = mapped.parse()?;
                        mapped.parse::<Token![:]>()?;
                        let path: syn::Type = mapped.parse()?;
                        target.insert(name.value(), path);
                        if !mapped.is_empty() {
                            mapped.parse::<Token![,]>()?;
                        }
                    }
                }
                "uses" => {
                    let mapped;
                    braced!(mapped in content);
                    while !mapped.is_empty() {
                        let name: LitStr = mapped.parse()?;
                        mapped.parse::<Token![:]>()?;
                        let path: syn::Path = mapped.parse()?;
                        options.uses.insert(name.value(), path);
                        if !mapped.is_empty() {
                            mapped.parse::<Token![,]>()?;
                        }
                    }
                }
                "copy" | "default" | "inline" | "lenient" | "ordered" | "deps" | "own_debug"
                | "local" => {
                    let names;
                    bracketed!(names in content);
                    let names: Punctuated<LitStr, Token![,]> =
                        names.parse_terminated(|input| input.parse(), Token![,])?;
                    let names = names.iter().map(LitStr::value).collect();
                    match key.to_string().as_str() {
                        "copy" => options.copy = names,
                        "inline" => options.inline = names,
                        "lenient" => options.lenient = names,
                        "ordered" => options.ordered = names,
                        "deps" => options.deps = names.into_iter().collect(),
                        "own_debug" => options.own_debug = names,
                        "local" => options.local = names,
                        _ => options.default = names,
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown option `{other}`"),
                    ));
                }
            }
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        Ok(options)
    }
}

mod local;

#[cfg(test)]
mod tests;

#[proc_macro]
pub fn generate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let options = syn::parse_macro_input!(input as Options);
    match Generator::run(options) {
        Ok(tokens) => tokens.into(),
        Err(error) => syn::Error::new(Span::call_site(), error)
            .to_compile_error()
            .into(),
    }
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield", "box", "try", "gen",
];

fn snake(name: &str) -> Ident {
    let name = name.to_snake_case();
    match KEYWORDS.contains(&name.as_str()) {
        true => Ident::new_raw(&name, Span::call_site()),
        false => Ident::new(&name, Span::call_site()),
    }
}

fn camel(name: &str) -> Ident {
    Ident::new(&name.to_upper_camel_case(), Span::call_site())
}

fn shouty(name: &str) -> Ident {
    Ident::new(&name.to_shouty_snake_case(), Span::call_site())
}

fn fnv(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

struct Generator {
    resolve: Resolve,
    options: Options,
    methods: BTreeMap<u32, String>,
    package: Option<wit_parser::PackageId>,
}

#[derive(Default)]
struct Modules {
    root: TokenStream,
    nested: BTreeMap<String, (Ident, TokenStream)>,
}

impl Modules {
    fn push(&mut self, nest: Option<Ident>, module: &Ident, items: TokenStream) {
        let item = quote! { pub mod #module { #items } };
        match nest {
            None => self.root.extend(item),
            Some(nest) => self
                .nested
                .entry(nest.to_string())
                .or_insert_with(|| (nest, TokenStream::new()))
                .1
                .extend(item),
        }
    }

    fn finish(self) -> TokenStream {
        let mut out = self.root;
        for (_, (nest, items)) in self.nested {
            out.extend(quote! { pub mod #nest { #items } });
        }
        out
    }
}

enum Shape {
    Call(Option<Type>),
    Stream(Option<Type>),
}

struct Method<'a> {
    function: &'a Function,
    name: String,
    interface: String,
    constant: Ident,
    args: Ident,
    params: Vec<(Ident, Type)>,
    shape: Shape,
}

impl Generator {
    fn run(options: Options) -> Result<TokenStream, String> {
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_owned())?;
        let path = PathBuf::from(&root).join(&options.path);
        let mut resolve = Resolve::new();
        for dep in &options.deps {
            let dep = PathBuf::from(&root).join(dep);
            resolve
                .push_dir(&dep)
                .map_err(|error| format!("{}: {error:#}", dep.display()))?;
        }
        let (package, _) = resolve
            .push_dir(&path)
            .map_err(|error| format!("{}: {error:#}", path.display()))?;
        Self::generate(options, resolve, package)
    }

    fn generate(
        options: Options,
        resolve: Resolve,
        package: wit_parser::PackageId,
    ) -> Result<TokenStream, String> {
        let world = resolve
            .select_world(&[package], Some(&options.world))
            .map_err(|error| format!("{error:#}"))?;
        let mut generator = Generator {
            resolve,
            options,
            methods: BTreeMap::new(),
            package: None,
        };
        let world = generator.resolve.worlds[world].clone();
        generator.package = world.package;
        let interfaces = |items: &wit_parser::IndexMap<_, WorldItem>| {
            items
                .values()
                .filter_map(|item| match item {
                    WorldItem::Interface { id, .. } => Some(*id),
                    _ => None,
                })
                .collect::<Vec<InterfaceId>>()
        };
        let imports = interfaces(&world.imports);
        let exports = interfaces(&world.exports);
        for name in &generator.options.local {
            let imported = imports
                .iter()
                .any(|interface| generator.resolve.id_of(*interface).as_ref() == Some(name));
            if !imported {
                return Err(format!("{name} is local but the world does not import it"));
            }
        }
        if let Some(exported) = exports
            .iter()
            .find(|interface| generator.is_local(**interface))
        {
            return Err(format!(
                "{}: a local interface is answered by the host, so it cannot be exported",
                generator.resolve.id_of(*exported).unwrap_or_default()
            ));
        }
        for interface in imports.iter().chain(&exports) {
            generator.register(*interface)?;
        }
        let generator = generator;
        let mut types = Modules::default();
        let mut defined = BTreeSet::new();
        for interface in imports.iter().chain(&exports) {
            generator.types_of(*interface, &mut defined, &mut types)?;
        }
        let types = types.finish();
        if generator.options.network {
            return generator.network(&imports, &exports, types);
        }
        let mut args = TokenStream::new();
        let mut guest_imports = Modules::default();
        let mut embed_imports = Modules::default();
        let mut watches = Vec::new();
        let mut named = Vec::new();
        let dollar = proc_macro2::Punct::new('$', proc_macro2::Spacing::Alone);
        let callable = |interface: &&InterfaceId| {
            !generator.resolve.interfaces[**interface]
                .functions
                .is_empty()
        };
        let mut local_guest = TokenStream::new();
        let mut local_embed = TokenStream::new();
        let mut local_entries = Vec::new();
        let mut served = Vec::new();
        for interface in imports.iter().filter(callable) {
            let module = generator.module_name(*interface);
            if generator.is_local(*interface) {
                if generator.used(*interface).is_some() {
                    continue;
                }
                let local = generator.local(*interface)?;
                let (guest, embed) = (local.guest, local.embed);
                local_guest.extend(quote! { pub mod #module { #guest } });
                local_embed.extend(quote! { pub mod #module { #embed } });
                local_entries.extend(local.entries);
                continue;
            }
            let methods = generator.methods(*interface)?;
            args.extend(generator.args(&module, &methods));
            watches.extend(generator.watches(&methods));
            named.extend(methods.iter().map(|method| {
                let (constant, name) = (&method.constant, &method.name);
                quote!((crate::__args::#constant, #name))
            }));
            let guest = generator.guest_import(&module, &methods)?;
            let embed = generator.embed_import(&module, &methods);
            let nest = generator.nest(*interface);
            served.push(*interface);
            guest_imports.push(nest.clone(), &module, guest);
            embed_imports.push(nest, &module, embed);
        }
        let world_call = generator.world_call(&served);
        let (guest_imports, embed_imports) = (guest_imports.finish(), embed_imports.finish());
        let mut guest_exports = Modules::default();
        let mut embed_exports = Modules::default();
        let mut bounds = Vec::new();
        let mut dispatches = Vec::new();
        for interface in exports.iter().filter(callable) {
            let module = generator.module_name(*interface);
            let methods = generator.methods(*interface)?;
            args.extend(generator.args(&module, &methods));
            let guest = generator.guest_export(*interface, &module, &methods)?;
            let embed = generator.embed_export(*interface, &module, &methods)?;
            let nest = generator.nest(*interface);
            guest_exports.push(nest.clone(), &module, guest);
            embed_exports.push(nest, &module, embed);
            let path = generator.module_path(*interface);
            bounds.push(quote! { exports::#path::Guest });
            dispatches.push(quote! {
                if let ::core::option::Option::Some(served) =
                    exports::#path::dispatch(&guest, resources, started.clone())
                {
                    return served;
                }
            });
        }
        let (guest_exports, embed_exports) = (guest_exports.finish(), embed_exports.finish());
        let lister = format_ident!("{}_local_imports", generator.options.world.to_snake_case());
        let listing = match local_entries.is_empty() {
            true => TokenStream::new(),
            false => quote! {
                #[macro_export]
                macro_rules! #lister {
                    (#dollar callback:ident) => {
                        #dollar callback! { #(#local_entries)* }
                    };
                }

                pub mod guest {
                    #local_guest
                }

                pub mod embed {
                    #local_embed
                }
            },
        };
        if bounds.is_empty() && args.is_empty() {
            return Ok(quote! {
                #types

                #listing
            });
        }
        if !local_entries.is_empty() {
            return Err(
                "a world that imports a local interface imports nothing else of its own; \
                 take it from its own crate with `uses`"
                    .to_owned(),
            );
        }
        Ok(quote! {
            #types

            #[doc(hidden)]
            pub mod __args {
                #args
            }

            pub mod guest {
                #guest_imports

                pub mod exports {
                    #guest_exports
                }

                pub fn executor<G: #(#bounds)+*>(guest: G) -> ::be_ops::guest::Executor {
                    let guest = ::std::rc::Rc::new(guest);
                    ::be_ops::guest::Executor::new(::std::boxed::Box::new(
                        move |resources, started| {
                            #(#dispatches)*
                            ::be_ops::guest::Serve::Done(::be_ops::Outcome::Err(
                                ::be_ops::OpError::Unsupported,
                            ))
                        },
                    ))
                }
            }

            pub mod embed {
                pub const WATCHES: &[::be_ops::Watching] = &[#(#watches),*];

                pub const METHODS: &[(::be_ops::MethodId, &str)] = &[#(#named),*];

                pub fn method_name(method: ::be_ops::MethodId) -> ::core::option::Option<&'static str> {
                    METHODS
                        .iter()
                        .find(|(id, _)| *id == method)
                        .map(|(_, name)| *name)
                }

                #embed_imports

                #world_call

                pub mod exports {
                    #embed_exports
                }
            }
        })
    }

    fn network(
        &self,
        imports: &[InterfaceId],
        exports: &[InterfaceId],
        types: TokenStream,
    ) -> Result<TokenStream, String> {
        if let Some(exported) = exports.first() {
            return Err(format!(
                "{}: a network world only imports: the server serves its imports and the \
                 client calls them",
                self.resolve.id_of(*exported).unwrap_or_default()
            ));
        }
        if !self.options.local.is_empty() {
            return Err("a network world has no local interfaces".to_owned());
        }
        let mut args = TokenStream::new();
        let mut embed = Modules::default();
        let mut client = Modules::default();
        let mut served = Vec::new();
        for interface in imports {
            if self.resolve.interfaces[*interface].functions.is_empty() {
                continue;
            }
            served.push(*interface);
            let module = self.module_name(*interface);
            let methods = self.methods(*interface)?;
            args.extend(self.args(&module, &methods));
            let nest = self.nest(*interface);
            embed.push(nest.clone(), &module, self.embed_import(&module, &methods));
            client.push(nest, &module, self.client(&methods)?);
        }
        let world_call = self.world_call(&served);
        let (embed, client) = (embed.finish(), client.finish());
        Ok(quote! {
            #types

            #[doc(hidden)]
            pub mod __args {
                #args
            }

            pub mod embed {
                #embed

                #world_call
            }

            pub mod client {
                #client
            }
        })
    }

    fn client(&self, methods: &[Method<'_>]) -> Result<TokenStream, String> {
        let caller = quote!(caller: &(impl ::be_ops::net::Caller + ::core::clone::Clone));
        let mut out = TokenStream::new();
        let mut resources = BTreeMap::<String, Vec<TokenStream>>::new();
        for method in methods {
            let name = snake(method.function.item_name());
            let params = self.params(method)?;
            let constant = &method.constant;
            let args = Self::construct(method);
            let resource = method.function.kind.resource().map(|resource| {
                self.resolve.types[resource]
                    .name
                    .clone()
                    .unwrap_or_default()
            });
            let (returns, start) = match &method.shape {
                Shape::Call(result) => {
                    let encoding = self.encoding(result.as_ref());
                    let result = self.optional(result.as_ref())?;
                    (
                        quote!(::be_ops::net::Pending<#result, #encoding>),
                        quote!(::be_ops::net::call),
                    )
                }
                Shape::Stream(item) => {
                    let encoding = self.encoding(item.as_ref());
                    let item = self.optional(item.as_ref())?;
                    (
                        quote!(::be_ops::net::Stream<#item, #encoding>),
                        quote!(::be_ops::net::stream),
                    )
                }
            };
            let Some(resource) = resource else {
                out.extend(quote! {
                    pub fn #name(#caller, #(#params),*) -> #returns {
                        #start(
                            ::be_ops::net::share(caller),
                            ::core::option::Option::None,
                            crate::__args::#constant,
                            &#args,
                        )
                    }
                });
                continue;
            };
            let item = match method.function.kind {
                FunctionKind::Constructor(_) => quote! {
                    pub fn new(#caller, #(#params),*) -> Self {
                        Self(::be_ops::net::open(
                            ::be_ops::net::share(caller),
                            crate::__args::#constant,
                            &#args,
                        ))
                    }
                },
                _ => quote! {
                    pub fn #name(&self, #(#params),*) -> #returns {
                        #start(
                            self.0.caller(),
                            ::core::option::Option::Some(self.0.op()),
                            crate::__args::#constant,
                            &#args,
                        )
                    }
                },
            };
            resources.entry(resource).or_default().push(item);
        }
        for (resource, items) in resources {
            let name = camel(&resource);
            out.extend(quote! {
                #[derive(Debug)]
                pub struct #name(::be_ops::net::Resource);

                impl #name {
                    #(#items)*

                    pub fn op(&self) -> ::be_ops::OpId {
                        self.0.op()
                    }

                    pub fn closed(&mut self) -> ::be_ops::net::Closed<'_> {
                        self.0.closed()
                    }

                    pub fn close(
                        self,
                    ) -> impl ::core::future::Future<
                        Output = ::core::result::Result<(), ::be_ops::OpError>,
                    > + ::core::marker::Send {
                        self.0.close()
                    }
                }
            });
        }
        Ok(out)
    }

    fn world_call(&self, interfaces: &[InterfaceId]) -> TokenStream {
        let modules: Vec<Ident> = interfaces.iter().map(|id| self.module_name(*id)).collect();
        let variants = interfaces.iter().zip(&modules).map(|(interface, module)| {
            let shared = modules.iter().filter(|other| *other == module).count() > 1;
            match (shared, self.nest(*interface)) {
                (true, Some(nest)) => {
                    format_ident!("{}{}", camel(&nest.to_string()), camel(&module.to_string()))
                }
                _ => camel(&module.to_string()),
            }
        });
        let (variants, paths): (Vec<Ident>, Vec<TokenStream>) = variants
            .zip(interfaces.iter().map(|id| self.module_path(*id)))
            .unzip();
        quote! {
            #[derive(Debug)]
            pub enum Call {
                #(#variants(#paths::Call),)*
            }

            impl Call {
                pub fn accept(
                    connection: &mut ::be_ops::Connection,
                    started: ::be_ops::Started,
                ) -> ::core::option::Option<Self> {
                    #(
                        if #paths::Call::accepts(&started) {
                            return #paths::Call::accept(connection, started).map(Self::#variants);
                        }
                    )*
                    connection.complete(
                        started.op,
                        ::be_ops::Outcome::Err(::be_ops::OpError::Unsupported),
                    );
                    ::core::option::Option::None
                }

                pub fn refuse(
                    self,
                    error: ::be_ops::OpError,
                ) -> (::be_ops::OpId, ::be_ops::Outcome) {
                    match self {
                        #(Self::#variants(call) => call.refuse(error),)*
                    }
                }
            }
        }
    }

    fn module_name(&self, interface: InterfaceId) -> Ident {
        snake(
            self.resolve.interfaces[interface]
                .name
                .as_deref()
                .unwrap_or("world"),
        )
    }

    fn nest(&self, interface: InterfaceId) -> Option<Ident> {
        let package = self.resolve.interfaces[interface].package?;
        if Some(package) == self.package {
            return None;
        }
        Some(snake(&self.resolve.packages[package].name.name))
    }

    fn module_path(&self, interface: InterfaceId) -> TokenStream {
        let module = self.module_name(interface);
        match self.nest(interface) {
            Some(nest) => quote!(#nest::#module),
            None => quote!(#module),
        }
    }

    fn used(&self, interface: InterfaceId) -> Option<&syn::Path> {
        self.options.uses.get(&self.resolve.id_of(interface)?)
    }

    fn mapped(&self, id: TypeId) -> Option<&syn::Type> {
        let name = self.resolve.types[id].name.as_ref()?;
        self.options.with.get(name)
    }

    fn target(&self, mut id: TypeId) -> TypeId {
        loop {
            let def = &self.resolve.types[id];
            match def.kind {
                TypeDefKind::Type(Type::Id(next)) if self.mapped(id).is_none() => id = next,
                _ => return id,
            }
        }
    }

    fn ty(&self, ty: &Type) -> Result<TokenStream, String> {
        Ok(match ty {
            Type::Bool => quote!(bool),
            Type::U8 => quote!(u8),
            Type::U16 => quote!(u16),
            Type::U32 => quote!(u32),
            Type::U64 => quote!(u64),
            Type::S8 => quote!(i8),
            Type::S16 => quote!(i16),
            Type::S32 => quote!(i32),
            Type::S64 => quote!(i64),
            Type::Char => quote!(char),
            Type::String => quote!(::std::string::String),
            Type::F32 => quote!(f32),
            Type::F64 => quote!(f64),
            Type::ErrorContext => {
                return Err(format!("{ty:?} is not supported over be-ops"));
            }
            Type::Id(id) => {
                let id = self.target(*id);
                let def = &self.resolve.types[id];
                if let (Some(name), TypeOwner::Interface(owner)) = (&def.name, def.owner)
                    && let Some(path) = self.used(owner)
                {
                    let name = camel(name);
                    return Ok(quote!(#path::#name));
                }
                if let Some(path) = self.mapped(id) {
                    return Ok(quote!(#path));
                }
                if let (Some(name), TypeOwner::Interface(owner)) = (&def.name, def.owner) {
                    let module = self.module_path(owner);
                    let name = camel(name);
                    return Ok(quote!(crate::#module::#name));
                }
                match &def.kind {
                    TypeDefKind::Option(inner) => {
                        let inner = self.ty(inner)?;
                        quote!(::core::option::Option<#inner>)
                    }
                    TypeDefKind::Result(result) => {
                        let ok = self.optional(result.ok.as_ref())?;
                        let err = self.optional(result.err.as_ref())?;
                        quote!(::core::result::Result<#ok, #err>)
                    }
                    TypeDefKind::List(inner) => {
                        let inner = self.ty(inner)?;
                        quote!(::std::vec::Vec<#inner>)
                    }
                    TypeDefKind::Tuple(tuple) => {
                        let types = tuple
                            .types
                            .iter()
                            .map(|ty| self.ty(ty))
                            .collect::<Result<Vec<_>, _>>()?;
                        quote!((#(#types,)*))
                    }
                    TypeDefKind::Handle(
                        wit_parser::Handle::Own(resource) | wit_parser::Handle::Borrow(resource),
                    ) => {
                        let resource = self.target(*resource);
                        let def = &self.resolve.types[resource];
                        match (&def.name, def.owner) {
                            (Some(name), TypeOwner::Interface(owner)) if self.is_local(owner) => {
                                let name = camel(name);
                                match self.used(owner) {
                                    Some(path) => quote!(#path::#name),
                                    None => {
                                        let module = self.module_path(owner);
                                        quote!(crate::#module::#name)
                                    }
                                }
                            }
                            _ => {
                                return Err(format!(
                                    "{def:?}: a handle is a value only in a local interface"
                                ));
                            }
                        }
                    }
                    other => return Err(format!("{other:?} is not supported here")),
                }
            }
        })
    }

    fn is_local(&self, interface: InterfaceId) -> bool {
        self.resolve
            .id_of(interface)
            .is_some_and(|id| self.options.local.contains(&id))
    }

    fn optional(&self, ty: Option<&Type>) -> Result<TokenStream, String> {
        match ty {
            Some(ty) => self.ty(ty),
            None => Ok(quote!(())),
        }
    }

    fn encoding_of(&self, ty: &Type) -> Option<TokenStream> {
        let Type::Id(id) = ty else {
            return None;
        };
        let id = self.target(*id);
        let def = &self.resolve.types[id];
        if let Some(shape) = def
            .name
            .as_ref()
            .and_then(|name| self.options.encodings.get(name))
        {
            return Some(quote!(#shape));
        }
        if self.mapped(id).is_some() {
            return None;
        }
        match &def.kind {
            TypeDefKind::List(Type::U8) => Some(quote!(::be_ops::wire::Bytes)),
            TypeDefKind::List(inner) => {
                let inner = self.encoding_of(inner)?;
                Some(quote!(::be_ops::wire::List<#inner>))
            }
            TypeDefKind::Option(inner) => {
                let inner = self.encoding_of(inner)?;
                Some(quote!(::be_ops::wire::Optional<#inner>))
            }
            TypeDefKind::Result(result) => {
                let ok = result.ok.as_ref().and_then(|ty| self.encoding_of(ty));
                let err = result.err.as_ref().and_then(|ty| self.encoding_of(ty));
                if ok.is_none() && err.is_none() {
                    return None;
                }
                let plain = || quote!(::be_ops::wire::Plain);
                let (ok, err) = (ok.unwrap_or_else(plain), err.unwrap_or_else(plain));
                Some(quote!(::be_ops::wire::Fallible<#ok, #err>))
            }
            TypeDefKind::Tuple(tuple) => {
                let shapes: Vec<_> = tuple.types.iter().map(|ty| self.encoding_of(ty)).collect();
                if shapes.iter().all(Option::is_none) {
                    return None;
                }
                let shapes = shapes
                    .into_iter()
                    .map(|shape| shape.unwrap_or_else(|| quote!(::be_ops::wire::Plain)));
                Some(quote!((#(#shapes,)*)))
            }
            TypeDefKind::Type(inner) => self.encoding_of(inner),
            _ => None,
        }
    }

    fn encoding(&self, ty: Option<&Type>) -> TokenStream {
        ty.and_then(|ty| self.encoding_of(ty))
            .unwrap_or_else(|| quote!(::be_ops::wire::Plain))
    }

    fn encoding_attribute(&self, ty: &Type) -> TokenStream {
        let Some(shape) = self.encoding_of(ty) else {
            return TokenStream::new();
        };
        let serialize = format!("::be_ops::wire::serialize::<{shape}, _, _>");
        let deserialize = format!("::be_ops::wire::deserialize::<{shape}, _, _>");
        quote!(#[serde(serialize_with = #serialize, deserialize_with = #deserialize)])
    }

    fn copyable(&self, ty: &Type) -> bool {
        match ty {
            Type::String => false,
            Type::Id(id) => {
                let id = self.target(*id);
                let def = &self.resolve.types[id];
                if self.mapped(id).is_some() {
                    return def
                        .name
                        .as_ref()
                        .is_some_and(|name| self.options.copy.contains(name));
                }
                match &def.kind {
                    TypeDefKind::Enum(_) | TypeDefKind::Flags(_) | TypeDefKind::Handle(_) => true,
                    TypeDefKind::Record(record) => {
                        record.fields.iter().all(|field| self.copyable(&field.ty))
                    }
                    TypeDefKind::Variant(variant) => variant
                        .cases
                        .iter()
                        .all(|case| case.ty.as_ref().is_none_or(|ty| self.copyable(ty))),
                    TypeDefKind::Tuple(tuple) => tuple.types.iter().all(|ty| self.copyable(ty)),
                    TypeDefKind::Option(inner) | TypeDefKind::Type(inner) => self.copyable(inner),
                    _ => false,
                }
            }
            _ => true,
        }
    }

    fn floating(&self, ty: &Type) -> bool {
        match ty {
            Type::F32 | Type::F64 => true,
            Type::Id(id) => {
                let id = self.target(*id);
                match &self.resolve.types[id].kind {
                    TypeDefKind::Record(record) => {
                        record.fields.iter().any(|field| self.floating(&field.ty))
                    }
                    TypeDefKind::Variant(variant) => variant
                        .cases
                        .iter()
                        .any(|case| case.ty.as_ref().is_some_and(|ty| self.floating(ty))),
                    TypeDefKind::Tuple(tuple) => tuple.types.iter().any(|ty| self.floating(ty)),
                    TypeDefKind::Option(inner)
                    | TypeDefKind::Type(inner)
                    | TypeDefKind::List(inner) => self.floating(inner),
                    TypeDefKind::Result(result) => {
                        result.ok.as_ref().is_some_and(|ty| self.floating(ty))
                            || result.err.as_ref().is_some_and(|ty| self.floating(ty))
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn derives(&self, id: TypeId) -> TokenStream {
        let name = self.type_name(id);
        let listed =
            |list: &BTreeSet<String>| name.as_ref().is_some_and(|name| list.contains(name));
        let lenient = match listed(&self.options.lenient) {
            true => quote!(#[serde(default)]),
            false => TokenStream::new(),
        };
        let copy = self.copyable(&Type::Id(id));
        let exact = !self.floating(&Type::Id(id));
        let ordered = exact && (copy || listed(&self.options.ordered));
        let mut derives = vec!["Clone"];
        derives.extend(copy.then_some("Copy"));
        derives.extend((!listed(&self.options.own_debug)).then_some("Debug"));
        derives.extend(listed(&self.options.default).then_some("Default"));
        derives.extend(exact.then_some("Eq"));
        derives.extend(ordered.then_some("Hash"));
        derives.extend(ordered.then_some("Ord"));
        derives.push("PartialEq");
        derives.extend(ordered.then_some("PartialOrd"));
        let derives = derives
            .into_iter()
            .map(|derive| Ident::new(derive, Span::call_site()));
        quote! {
            #[derive(#(#derives,)* ::serde::Serialize, ::serde::Deserialize)]
            #lenient
        }
    }

    fn inlined(&self, ty: &Type) -> Option<&wit_parser::Record> {
        let Type::Id(id) = ty else {
            return None;
        };
        let def = &self.resolve.types[*id];
        let name = def.name.as_ref()?;
        if !self.options.inline.contains(name) {
            return None;
        }
        match &def.kind {
            TypeDefKind::Record(record) => Some(record),
            _ => None,
        }
    }

    fn fields(
        &self,
        record: &wit_parser::Record,
        public: bool,
    ) -> Result<Vec<TokenStream>, String> {
        record
            .fields
            .iter()
            .map(|field| {
                let name = snake(&field.name);
                let ty = self.ty(&field.ty)?;
                let bytes = self.encoding_attribute(&field.ty);
                Ok(match public {
                    true => quote! { #bytes pub #name: #ty, },
                    false => quote! { #bytes #name: #ty, },
                })
            })
            .collect()
    }

    fn type_name(&self, id: TypeId) -> Option<String> {
        self.resolve.types[id].name.clone()
    }

    fn types_of(
        &self,
        interface: InterfaceId,
        defined: &mut BTreeSet<InterfaceId>,
        out: &mut Modules,
    ) -> Result<(), String> {
        if !defined.insert(interface) || self.used(interface).is_some() {
            return Ok(());
        }
        let mut items = TokenStream::new();
        let owned = self.resolve.interfaces[interface].types.clone();
        for (name, id) in owned {
            let def = &self.resolve.types[id];
            if let TypeDefKind::Type(Type::Id(target)) = def.kind
                && let TypeOwner::Interface(other) = self.resolve.types[target].owner
                && other != interface
            {
                self.types_of(other, defined, out)?;
                continue;
            }
            if let Some(mapped) = self.options.with.get(&name) {
                let ident = camel(&name);
                items.extend(quote! { pub type #ident = #mapped; });
                continue;
            }
            if self.options.inline.contains(&name) {
                continue;
            }
            let ident = camel(&name);
            let derives = self.derives(id);
            items.extend(match &def.kind {
                TypeDefKind::Record(record) => {
                    let fields = self.fields(record, true)?;
                    quote! { #derives pub struct #ident { #(#fields)* } }
                }
                TypeDefKind::Variant(variant) => {
                    let defaulted = self.options.default.contains(&name);
                    let cases = variant
                        .cases
                        .iter()
                        .enumerate()
                        .map(|(index, case)| {
                            let name = camel(&case.name);
                            let first = match defaulted && index == 0 {
                                true => quote!(#[default]),
                                false => TokenStream::new(),
                            };
                            Ok(match &case.ty {
                                Some(ty) => match self.inlined(ty) {
                                    Some(record) => {
                                        let fields = self.fields(record, false)?;
                                        quote! { #name { #(#fields)* }, }
                                    }
                                    None => {
                                        let bytes = self.encoding_attribute(ty);
                                        let ty = self.ty(ty)?;
                                        quote! { #name(#bytes #ty), }
                                    }
                                },
                                None => quote! { #first #name, },
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    quote! { #derives pub enum #ident { #(#cases)* } }
                }
                TypeDefKind::Enum(cases) => {
                    let first = match self.options.default.contains(&name) {
                        true => quote!(#[default]),
                        false => TokenStream::new(),
                    };
                    let mut cases = cases.cases.iter().map(|case| camel(&case.name));
                    let head = cases.next();
                    quote! { #derives pub enum #ident { #first #head, #(#cases,)* } }
                }
                TypeDefKind::Flags(flags) => {
                    if flags.flags.len() > 32 {
                        return Err(format!("{name} has more than 32 flags"));
                    }
                    let constants = flags.flags.iter().enumerate().map(|(index, flag)| {
                        let flag = shouty(&flag.name);
                        let bit = 1u32 << index;
                        quote! { pub const #flag: Self = Self(#bit); }
                    });
                    quote! {
                        #derives
                        pub struct #ident(u32);

                        impl #ident {
                            #(#constants)*

                            pub const fn empty() -> Self {
                                Self(0)
                            }

                            pub const fn contains(self, other: Self) -> bool {
                                self.0 & other.0 == other.0
                            }

                            pub const fn bits(self) -> u32 {
                                self.0
                            }

                            pub const fn from_bits(bits: u32) -> Self {
                                Self(bits)
                            }
                        }

                        impl ::core::ops::BitOr for #ident {
                            type Output = Self;

                            fn bitor(self, other: Self) -> Self {
                                Self(self.0 | other.0)
                            }
                        }
                    }
                }
                TypeDefKind::Resource if self.is_local(interface) => quote! {
                    #[derive(
                        Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
                        ::serde::Serialize, ::serde::Deserialize,
                    )]
                    #[serde(transparent)]
                    pub struct #ident(pub u32);
                },
                TypeDefKind::Resource => TokenStream::new(),
                TypeDefKind::Type(_)
                | TypeDefKind::Option(_)
                | TypeDefKind::Result(_)
                | TypeDefKind::List(_)
                | TypeDefKind::Tuple(_) => {
                    let ty = self.anonymous(&def.kind)?;
                    quote! { pub type #ident = #ty; }
                }
                other => return Err(format!("{name}: {other:?} is not supported over be-ops")),
            });
        }
        let module = self.module_name(interface);
        out.push(self.nest(interface), &module, items);
        Ok(())
    }

    fn anonymous(&self, kind: &TypeDefKind) -> Result<TokenStream, String> {
        match kind {
            TypeDefKind::Type(ty) => self.ty(ty),
            TypeDefKind::Option(inner) => {
                let inner = self.ty(inner)?;
                Ok(quote!(::core::option::Option<#inner>))
            }
            TypeDefKind::Result(result) => {
                let ok = self.optional(result.ok.as_ref())?;
                let err = self.optional(result.err.as_ref())?;
                Ok(quote!(::core::result::Result<#ok, #err>))
            }
            TypeDefKind::List(inner) => {
                let inner = self.ty(inner)?;
                Ok(quote!(::std::vec::Vec<#inner>))
            }
            TypeDefKind::Tuple(tuple) => {
                let types = tuple
                    .types
                    .iter()
                    .map(|ty| self.ty(ty))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(quote!((#(#types,)*)))
            }
            other => Err(format!("{other:?} is not supported here")),
        }
    }

    fn register(&mut self, interface: InterfaceId) -> Result<(), String> {
        let id = self
            .resolve
            .id_of(interface)
            .ok_or("an interface of the world has no name")?;
        for key in self.resolve.interfaces[interface].functions.keys() {
            let full = format!("{id}#{key}");
            if let Some(other) = self.methods.insert(fnv(&full), full.clone()) {
                return Err(format!("{full} and {other} hash to the same method id"));
            }
        }
        Ok(())
    }

    fn methods(&self, interface: InterfaceId) -> Result<Vec<Method<'_>>, String> {
        let id = self
            .resolve
            .id_of(interface)
            .ok_or("an interface of the world has no name")?;
        let module = match self.nest(interface) {
            Some(nest) => format!("{nest}_{}", self.module_name(interface)),
            None => self.module_name(interface).to_string(),
        };
        let mut methods = Vec::new();
        let interface = &self.resolve.interfaces[interface];
        for (key, function) in &interface.functions {
            let full = format!("{id}#{key}");
            let skip = matches!(
                function.kind,
                FunctionKind::Method(_) | FunctionKind::AsyncMethod(_)
            ) as usize;
            let params = function
                .params
                .iter()
                .skip(skip)
                .map(|param| (snake(&param.name), param.ty))
                .collect();
            let stream = match function.result {
                Some(Type::Id(result)) => match &self.resolve.types[result].kind {
                    TypeDefKind::Stream(item) => Some(*item),
                    TypeDefKind::Future(_) => {
                        return Err(format!("{full}: future<T> is not supported yet"));
                    }
                    _ => None,
                },
                _ => None,
            };
            let result = match function.kind {
                FunctionKind::Constructor(_) => None,
                _ => function.result,
            };
            let shape = match (stream, function.kind.is_async(), result) {
                (Some(item), false, _) => Shape::Stream(item),
                (Some(_), true, _) => {
                    return Err(format!("{full}: a stream is returned by a plain func"));
                }
                (None, true, result) => Shape::Call(result),
                (None, false, _) if matches!(function.kind, FunctionKind::Constructor(_)) => {
                    Shape::Call(None)
                }
                (None, false, _) => {
                    return Err(format!(
                        "{full}: a func is answered on the caller's thread, which only a local \
                         interface does; make it an async func"
                    ));
                }
            };
            if matches!(
                function.kind,
                FunctionKind::Static(_) | FunctionKind::AsyncStatic(_)
            ) {
                return Err(format!(
                    "{full}: static resource functions are not supported"
                ));
            }
            let local = function.item_name().to_owned();
            let prefix = match function.kind.resource() {
                Some(resource) => format!(
                    "{}_",
                    self.resolve.types[resource]
                        .name
                        .as_deref()
                        .unwrap_or("resource")
                ),
                None => String::new(),
            };
            let local = match function.kind {
                FunctionKind::Constructor(_) => format!("{prefix}new"),
                _ => format!("{prefix}{local}"),
            };
            methods.push(Method {
                function,
                name: full,
                interface: id.clone(),
                constant: shouty(&format!("{module}_{local}")),
                args: camel(&format!("{module}_{local}_args")),
                params,
                shape,
            });
        }
        Ok(methods)
    }

    fn watched<'a>(
        &self,
        methods: &'a [Method<'a>],
    ) -> Vec<(String, &'a Method<'a>, &'a Method<'a>, &'a Type)> {
        let mut watched = Vec::new();
        let mut resources = BTreeMap::<TypeId, Vec<&Method<'_>>>::new();
        for method in methods {
            if let Some(resource) = method.function.kind.resource() {
                resources.entry(resource).or_default().push(method);
            }
        }
        for (resource, methods) in resources {
            let name = self.resolve.types[resource]
                .name
                .clone()
                .unwrap_or_default();
            if !name.ends_with("-watch") {
                continue;
            }
            let open = methods.iter().find(|method| {
                matches!(method.function.kind, FunctionKind::Constructor(_))
                    && method.params.is_empty()
            });
            let next = methods.iter().find(|method| {
                matches!(method.function.kind, FunctionKind::AsyncMethod(_))
                    && method.function.item_name() == "next"
                    && method.params.is_empty()
            });
            if let (2, Some(open), Some(next)) = (methods.len(), open, next)
                && let Shape::Call(Some(value)) = &next.shape
            {
                watched.push((name, *open, *next, value));
            }
        }
        watched
    }

    fn watches(&self, methods: &[Method<'_>]) -> Vec<TokenStream> {
        self.watched(methods)
            .into_iter()
            .map(|(_, open, next, _)| {
                let interface = &open.interface;
                let (open, next) = (&open.constant, &next.constant);
                quote! {
                    ::be_ops::Watching {
                        interface: #interface,
                        open: crate::__args::#open,
                        next: crate::__args::#next,
                    }
                }
            })
            .collect()
    }

    fn args(&self, _module: &Ident, methods: &[Method<'_>]) -> TokenStream {
        let mut out = TokenStream::new();
        for method in methods {
            let constant = &method.constant;
            let name = &method.name;
            let args = &method.args;
            let fields = method.params.iter().map(|(name, ty)| {
                let bytes = self.encoding_attribute(ty);
                let ty = self.ty(ty).unwrap_or_else(|error| {
                    syn::Error::new(Span::call_site(), error).to_compile_error()
                });
                quote! { #bytes pub #name: #ty, }
            });
            out.extend(quote! {
                pub const #constant: ::be_ops::MethodId = ::be_ops::method_id(#name);

                #[derive(::serde::Serialize, ::serde::Deserialize)]
                pub struct #args { #(#fields)* }
            });
        }
        out
    }

    fn params(&self, method: &Method<'_>) -> Result<Vec<TokenStream>, String> {
        method
            .params
            .iter()
            .map(|(name, ty)| {
                let ty = self.ty(ty)?;
                Ok(quote!(#name: #ty))
            })
            .collect()
    }

    fn construct(method: &Method<'_>) -> TokenStream {
        let args = &method.args;
        let names = method.params.iter().map(|(name, _)| name);
        quote!(crate::__args::#args { #(#names),* })
    }

    fn guest_import(&self, _module: &Ident, methods: &[Method<'_>]) -> Result<TokenStream, String> {
        let mut out = TokenStream::new();
        let mut resources = BTreeMap::<String, Vec<TokenStream>>::new();
        for method in methods {
            let name = snake(method.function.item_name());
            let params = self.params(method)?;
            let constant = &method.constant;
            let args = Self::construct(method);
            if let Some(resource) = method.function.kind.resource() {
                let resource = self.resolve.types[resource]
                    .name
                    .clone()
                    .unwrap_or_default();
                let item = match (&method.function.kind, &method.shape) {
                    (FunctionKind::Constructor(_), _) => quote! {
                        pub fn new(#(#params),*) -> Self {
                            Self(::be_ops::guest::open(crate::__args::#constant, &#args))
                        }
                    },
                    (_, Shape::Call(result)) => {
                        let encoding = self.encoding(result.as_ref());
                        let result = self.optional(result.as_ref())?;
                        quote! {
                            pub fn #name(&self, #(#params),*) -> ::be_ops::guest::Pending<#result, #encoding> {
                                ::be_ops::guest::with_parent(self.0.op(), || {
                                    ::be_ops::guest::call(crate::__args::#constant, &#args)
                                })
                            }
                        }
                    }
                    (_, Shape::Stream(item)) => {
                        let encoding = self.encoding(item.as_ref());
                        let item = self.optional(item.as_ref())?;
                        quote! {
                            pub fn #name(&self, #(#params),*) -> ::be_ops::guest::Stream<#item, #encoding> {
                                ::be_ops::guest::with_parent(self.0.op(), || {
                                    ::be_ops::guest::stream(crate::__args::#constant, &#args)
                                })
                            }
                        }
                    }
                };
                resources.entry(resource).or_default().push(item);
                continue;
            }
            out.extend(match &method.shape {
                Shape::Call(result) => {
                    let encoding = self.encoding(result.as_ref());
                    let result = self.optional(result.as_ref())?;
                    quote! {
                        pub fn #name(#(#params),*) -> ::be_ops::guest::Pending<#result, #encoding> {
                            ::be_ops::guest::call(crate::__args::#constant, &#args)
                        }
                    }
                }
                Shape::Stream(item) => {
                    let encoding = self.encoding(item.as_ref());
                    let item = self.optional(item.as_ref())?;
                    quote! {
                        pub fn #name(#(#params),*) -> ::be_ops::guest::Stream<#item, #encoding> {
                            ::be_ops::guest::stream(crate::__args::#constant, &#args)
                        }
                    }
                }
            });
        }
        for (resource, open, next, value) in self.watched(methods) {
            if self.encoding_of(value).is_some() {
                return Err(format!(
                    "{}: a watched value is a record, a variant or a type without bytes in it",
                    next.name
                ));
            }
            let name = camel(&resource);
            let value = self.ty(value)?;
            let interface = &open.interface;
            let (open, next) = (&open.constant, &next.constant);
            out.extend(quote! {
                impl ::be_ops::Watched for #name {
                    type Value = #value;

                    const INTERFACE: &'static str = #interface;
                    const OPEN: ::be_ops::MethodId = crate::__args::#open;
                    const NEXT: ::be_ops::MethodId = crate::__args::#next;

                    fn open() -> Self {
                        Self::new()
                    }

                    fn next(&self) -> ::be_ops::guest::Pending<#value> {
                        #name::next(self)
                    }
                }
            });
        }
        for (resource, items) in resources {
            let name = camel(&resource);
            out.extend(quote! {
                pub struct #name(::be_ops::guest::Resource);

                impl #name {
                    #(#items)*

                    pub fn op(&self) -> ::be_ops::OpId {
                        self.0.op()
                    }

                    pub fn ended(&self) -> bool {
                        self.0.ended()
                    }
                }
            });
        }
        Ok(out)
    }

    fn embed_import(&self, _module: &Ident, methods: &[Method<'_>]) -> TokenStream {
        let mut cases = Vec::new();
        let mut started_methods: Vec<TokenStream> = Vec::new();
        let mut arms = Vec::new();
        let mut refusals = Vec::new();
        let mut handles = BTreeSet::new();
        for method in methods {
            let resource = method.function.kind.resource().map(|resource| {
                camel(
                    &self.resolve.types[resource]
                        .name
                        .clone()
                        .unwrap_or_default(),
                )
            });
            let case = match (&resource, &method.function.kind) {
                (Some(resource), FunctionKind::Constructor(_)) => format_ident!("{resource}New"),
                (Some(resource), _) => {
                    format_ident!("{resource}{}", camel(method.function.item_name()))
                }
                (None, _) => camel(method.function.item_name()),
            };
            if let Some(resource) = &resource {
                handles.insert(resource.to_string());
            }
            let constant = &method.constant;
            let args = &method.args;
            let fields: Vec<TokenStream> = method
                .params
                .iter()
                .map(|(name, ty)| {
                    let ty = self.ty(ty).unwrap_or_else(|error| {
                        syn::Error::new(Span::call_site(), error).to_compile_error()
                    });
                    quote!(#name: #ty)
                })
                .collect();
            let names: Vec<&Ident> = method.params.iter().map(|(name, _)| name).collect();
            let (field, value) = match (&method.shape, &resource, &method.function.kind) {
                (_, Some(resource), FunctionKind::Constructor(_)) => (
                    Some(quote!(resource: #resource)),
                    Some(quote!(resource: #resource(op))),
                ),
                (Shape::Call(result), _, _) => {
                    let encoding = self.encoding(result.as_ref());
                    let result = self.optional(result.as_ref()).unwrap_or_else(|error| {
                        syn::Error::new(Span::call_site(), error).to_compile_error()
                    });
                    (
                        Some(quote!(reply: ::be_ops::Reply<#result, #encoding>)),
                        Some(quote!(reply: ::be_ops::Reply::new(connection, op))),
                    )
                }
                (Shape::Stream(item), _, _) => {
                    let encoding = self.encoding(item.as_ref());
                    let item = self.optional(item.as_ref()).unwrap_or_else(|error| {
                        syn::Error::new(Span::call_site(), error).to_compile_error()
                    });
                    (
                        Some(quote!(stream: ::be_ops::Producer<#item, #encoding>)),
                        Some(quote!(stream: ::be_ops::Producer::new(op))),
                    )
                }
            };
            let method_of = matches!(
                method.function.kind,
                FunctionKind::Method(_) | FunctionKind::AsyncMethod(_)
            );
            let (this, this_started) = match (&resource, method_of) {
                (Some(resource), true) => (
                    Some(quote!(this: #resource)),
                    Some(quote!(this: #resource(started.parent?))),
                ),
                _ => (None, None),
            };
            let (this, this_started) = (this.into_iter(), this_started.into_iter());
            let field = field.into_iter();
            let value = value.into_iter();
            cases.push(quote! { #case { #(#this,)* #(#field,)* #(#fields,)* } });
            refusals.push(match (&method.shape, &resource, &method.function.kind) {
                (_, Some(_), FunctionKind::Constructor(_)) => {
                    quote!(Call::#case { resource, .. } => resource.op())
                }
                (Shape::Call(_), _, _) => quote!(Call::#case { reply, .. } => reply.into_op()),
                (Shape::Stream(_), _, _) => quote!(Call::#case { stream, .. } => stream.op()),
            });
            started_methods.push(quote!(crate::__args::#constant));
            arms.push(quote! {
                crate::__args::#constant => {
                    match ::be_ops::decode::<crate::__args::#args>(&started.args) {
                        ::core::option::Option::Some(crate::__args::#args { #(#names),* }) => {
                            Call::#case { #(#this_started,)* #(#value,)* #(#names,)* }
                        }
                        ::core::option::Option::None => {
                            connection.complete(
                                op,
                                ::be_ops::Outcome::Err(::be_ops::OpError::Unsupported),
                            );
                            return ::core::option::Option::None;
                        }
                    }
                }
            });
        }
        let handles = handles.into_iter().map(|name| {
            let name = Ident::new(&name, Span::call_site());
            quote! {
                #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
                pub struct #name(pub ::be_ops::OpId);

                impl #name {
                    pub fn op(self) -> ::be_ops::OpId {
                        self.0
                    }

                    pub fn end(self, connection: &mut ::be_ops::Connection) {
                        connection.complete(self.0, ::be_ops::Outcome::returned(&()));
                    }
                }
            }
        });
        let accept = match arms.is_empty() {
            true => quote! {
                connection.complete(
                    started.op,
                    ::be_ops::Outcome::Err(::be_ops::OpError::Unsupported),
                );
                ::core::option::Option::None
            },
            false => quote! {
                let op = started.op;
                ::core::option::Option::Some(match started.method {
                    #(#arms)*
                    _ => {
                        connection.complete(
                            op,
                            ::be_ops::Outcome::Err(::be_ops::OpError::Unsupported),
                        );
                        return ::core::option::Option::None;
                    }
                })
            },
        };
        quote! {
            #(#handles)*

            #[derive(Debug)]
            pub enum Call {
                #(#cases,)*
            }

            impl Call {
                pub fn accept(
                    connection: &mut ::be_ops::Connection,
                    started: ::be_ops::Started,
                ) -> ::core::option::Option<Self> {
                    #accept
                }

                pub fn accepts(started: &::be_ops::Started) -> bool {
                    let _ = started;
                    false #(|| started.method == #started_methods)*
                }

                pub fn refuse(
                    self,
                    error: ::be_ops::OpError,
                ) -> (::be_ops::OpId, ::be_ops::Outcome) {
                    let op = match self {
                        #(#refusals,)*
                    };
                    (op, ::be_ops::Outcome::Err(error))
                }
            }
        }
    }

    fn resources(&self, interface: InterfaceId) -> Vec<(TypeId, String)> {
        self.resolve.interfaces[interface]
            .types
            .iter()
            .filter(|(_, id)| matches!(self.resolve.types[**id].kind, TypeDefKind::Resource))
            .map(|(name, id)| (*id, name.clone()))
            .collect()
    }

    fn result_of(&self, method: &Method<'_>) -> Result<TokenStream, String> {
        match &method.shape {
            Shape::Call(result) => self.optional(result.as_ref()),
            Shape::Stream(_) => Err(format!(
                "{}: exported streams are not supported",
                method.name
            )),
        }
    }

    fn guest_export(
        &self,
        interface: InterfaceId,
        _module: &Ident,
        methods: &[Method<'_>],
    ) -> Result<TokenStream, String> {
        let resources = self.resources(interface);
        let mut guest_items = Vec::new();
        let mut resource_traits = BTreeMap::<TypeId, Vec<TokenStream>>::new();
        let mut arms = Vec::new();
        for (id, name) in &resources {
            let associated = camel(name);
            let bound = format_ident!("Guest{}", camel(name));
            guest_items.push(quote! { type #associated: #bound; });
            resource_traits.entry(*id).or_default();
        }
        for method in methods {
            let params = self.params(method)?;
            let names: Vec<&Ident> = method.params.iter().map(|(name, _)| name).collect();
            let constant = &method.constant;
            let args = &method.args;
            let result = self.result_of(method)?;
            let signature_result =
                quote!(-> impl ::core::future::Future<Output = #result> + 'static);
            let encoding = match &method.shape {
                Shape::Call(result) => self.encoding(result.as_ref()),
                Shape::Stream(_) => quote!(::be_ops::wire::Plain),
            };
            let serve = |call: TokenStream| {
                quote! {
                    let future = #call;
                    ::be_ops::guest::Serve::Later(::std::boxed::Box::pin(async move {
                        ::be_ops::Outcome::Ok(::be_ops::wire::encode::<#encoding, _>(&future.await))
                    }))
                }
            };
            let decode = |body: TokenStream| {
                quote! {
                    crate::__args::#constant => ::core::option::Option::Some(
                        match ::be_ops::decode::<crate::__args::#args>(&started.args) {
                            ::core::option::Option::Some(crate::__args::#args { #(#names),* }) => {
                                #body
                            }
                            ::core::option::Option::None => ::be_ops::guest::Serve::Done(
                                ::be_ops::Outcome::Err(::be_ops::OpError::Unsupported),
                            ),
                        },
                    ),
                }
            };
            match method.function.kind {
                FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => {
                    let name = snake(method.function.item_name());
                    guest_items.push(quote! { fn #name(&self, #(#params),*) #signature_result; });
                    arms.push(decode(serve(quote!(guest.#name(#(#names),*)))));
                }
                FunctionKind::Constructor(resource) => {
                    let type_name = self.resolve.types[resource]
                        .name
                        .clone()
                        .unwrap_or_default();
                    let associated = camel(&type_name);
                    let name = format_ident!("new_{}", type_name.to_snake_case());
                    guest_items.push(quote! {
                        fn #name(&self, resource: ::be_ops::OpId, #(#params),*) -> Self::#associated;
                    });
                    let bound = format_ident!("Guest{}", camel(&type_name));
                    arms.push(decode(quote! {
                        let instance = ::std::rc::Rc::new(guest.#name(started.op, #(#names),*));
                        let closing = ::std::rc::Rc::clone(&instance);
                        ::be_ops::guest::Serve::Resource {
                            instance,
                            on_drop: ::std::boxed::Box::new(move || {
                                ::std::boxed::Box::pin(#bound::on_drop(&*closing))
                            }),
                        }
                    }));
                }
                FunctionKind::Method(resource) | FunctionKind::AsyncMethod(resource) => {
                    let type_name = self.resolve.types[resource]
                        .name
                        .clone()
                        .unwrap_or_default();
                    let associated = camel(&type_name);
                    let name = snake(method.function.item_name());
                    resource_traits
                        .entry(resource)
                        .or_default()
                        .push(quote! { fn #name(&self, #(#params),*) #signature_result; });
                    let call = serve(quote!(this.#name(#(#names),*)));
                    arms.push(decode(quote! {
                        match resources.get::<G::#associated>(started.parent) {
                            ::core::option::Option::Some(this) => { #call }
                            ::core::option::Option::None => ::be_ops::guest::Serve::Done(
                                ::be_ops::Outcome::Err(::be_ops::OpError::Canceled),
                            ),
                        }
                    }));
                }
                FunctionKind::Static(_) | FunctionKind::AsyncStatic(_) => {
                    return Err(format!(
                        "{}: static functions are not supported",
                        method.name
                    ));
                }
            }
        }
        let traits = resource_traits.into_iter().map(|(id, items)| {
            let name = self.resolve.types[id].name.clone().unwrap_or_default();
            let bound = format_ident!("Guest{}", camel(&name));
            quote! {
                pub trait #bound: 'static {
                    #(#items)*

                    fn on_drop(&self) -> impl ::core::future::Future<Output = ()> + 'static;
                }
            }
        });
        Ok(quote! {
            pub trait Guest: 'static {
                #(#guest_items)*
            }

            #(#traits)*

            pub fn dispatch<G: Guest>(
                guest: &::std::rc::Rc<G>,
                resources: &::be_ops::guest::Resources<'_>,
                started: ::be_ops::Started,
            ) -> ::core::option::Option<::be_ops::guest::Serve> {
                let _ = (guest, resources);
                match started.method {
                    #(#arms)*
                    _ => ::core::option::Option::None,
                }
            }
        })
    }

    fn starting(method: &Method<'_>, parent: TokenStream, args: &TokenStream) -> TokenStream {
        let constant = &method.constant;
        quote! {
            let op = connection.start(#parent, crate::__args::#constant, ::be_ops::encode(&#args));
        }
    }

    fn host_returns(
        &self,
        method: &Method<'_>,
        result: &TokenStream,
    ) -> (TokenStream, TokenStream) {
        let (Shape::Call(ty) | Shape::Stream(ty)) = &method.shape;
        let encoding = self.encoding(ty.as_ref());
        (
            quote!(-> ::be_ops::Ticket<#result, #encoding>),
            quote!(::be_ops::Ticket::new(op)),
        )
    }

    fn embed_export(
        &self,
        interface: InterfaceId,
        _module: &Ident,
        methods: &[Method<'_>],
    ) -> Result<TokenStream, String> {
        let mut free = Vec::new();
        let mut impls = BTreeMap::<TypeId, Vec<TokenStream>>::new();
        for (id, _) in self.resources(interface) {
            impls.entry(id).or_default();
        }
        for method in methods {
            let params = self.params(method)?;
            let constant = &method.constant;
            let args = Self::construct(method);
            let result = self.result_of(method)?;
            match method.function.kind {
                FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => {
                    let name = snake(method.function.item_name());
                    let (returns, wrap) = self.host_returns(method, &result);
                    let starting =
                        Self::starting(method, quote!(::core::option::Option::None), &args);
                    free.push(quote! {
                        pub fn #name(
                            connection: &mut ::be_ops::Connection,
                            #(#params),*
                        ) #returns {
                            #starting
                            #wrap
                        }
                    });
                }
                FunctionKind::Constructor(resource) => {
                    impls.entry(resource).or_default().push(quote! {
                        pub fn new(connection: &mut ::be_ops::Connection, #(#params),*) -> Self {
                            Self(connection.start(
                                ::core::option::Option::None,
                                crate::__args::#constant,
                                ::be_ops::encode(&#args),
                            ))
                        }
                    });
                }
                FunctionKind::Method(resource) | FunctionKind::AsyncMethod(resource) => {
                    let name = snake(method.function.item_name());
                    let (returns, wrap) = self.host_returns(method, &result);
                    let starting =
                        Self::starting(method, quote!(::core::option::Option::Some(self.0)), &args);
                    impls.entry(resource).or_default().push(quote! {
                        pub fn #name(
                            self,
                            connection: &mut ::be_ops::Connection,
                            #(#params),*
                        ) #returns {
                            #starting
                            #wrap
                        }
                    });
                }
                FunctionKind::Static(_) | FunctionKind::AsyncStatic(_) => {
                    return Err(format!(
                        "{}: static functions are not supported",
                        method.name
                    ));
                }
            }
        }
        let resources = impls.into_iter().map(|(id, items)| {
            let name = camel(&self.resolve.types[id].name.clone().unwrap_or_default());
            quote! {
                #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
                pub struct #name(::be_ops::OpId);

                impl #name {
                    #(#items)*

                    pub fn from_op(op: ::be_ops::OpId) -> Self {
                        Self(op)
                    }

                    pub fn op(self) -> ::be_ops::OpId {
                        self.0
                    }

                    pub fn resource_drop(self, connection: &mut ::be_ops::Connection) {
                        connection.cancel(self.0);
                    }
                }
            }
        });
        Ok(quote! {
            #(#free)*

            #(#resources)*
        })
    }
}
