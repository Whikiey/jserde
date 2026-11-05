//! `#[derive(JavaSerializable)]` for `jser`.
//!
//! Attributes:
//! - `#[java(class = "com.example.Foo", suid = 1)]` on a struct (both required)
//! - `#[java(class = "com.example.Color")]` on a unit-only enum (SUID defaults to 0)
//! - `#[java(rename = "javaField")]` on a field or enum variant
//! - `#[java(transient)]` on a field (not serialized; defaults on decode)
//! - `#[java(flatten)]` on a struct field (superclass layer)
//!
//! Supported field types: Java primitives, `String`, `Option<T>`, `Vec<T>`
//! (arrays), `HashMap<K, V>`, `HashSet<T>` and nested `JavaSerializable` types.
//! Unit-only Rust enums map to Java enums (`TC_ENUM`, flags `0x12`, SUID 0);
//! variant names default to UPPER_SNAKE_CASE. Derived types must implement
//! `Clone` because the encoder converts fields by value.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Field, Fields, GenericArgument,
    GenericParam, LitInt, LitStr, PathArguments, Type,
};

#[proc_macro_derive(JavaSerializable, attributes(java))]
pub fn derive_java_serializable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct JavaAttrs {
    class: Option<String>,
    suid: Option<i64>,
    rename: Option<String>,
    transient: bool,
    flatten: bool,
}

fn parse_java_attrs(attrs: &[Attribute]) -> syn::Result<JavaAttrs> {
    let mut result = JavaAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("java") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("class") {
                let value: LitStr = meta.value()?.parse()?;
                result.class = Some(value.value());
            } else if meta.path.is_ident("suid") {
                let value: LitInt = meta.value()?.parse()?;
                result.suid = Some(value.base10_parse()?);
            } else if meta.path.is_ident("rename") {
                let value: LitStr = meta.value()?.parse()?;
                result.rename = Some(value.value());
            } else if meta.path.is_ident("transient") {
                result.transient = true;
            } else if meta.path.is_ident("flatten") {
                result.flatten = true;
            } else {
                return Err(meta.error("unknown `java` attribute"));
            }
            Ok(())
        })?;
    }

    Ok(result)
}

struct FieldInfo {
    ident: syn::Ident,
    java_name: String,
    typecode: proc_macro2::TokenStream,
    class_name1: Option<proc_macro2::TokenStream>,
    primitive: bool,
    vec: bool,
    optional_vec: bool,
    array_descriptor: Option<proc_macro2::TokenStream>,
    transient: bool,
    flatten: bool,
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if input
        .generics
        .params
        .iter()
        .any(|param| !matches!(param, GenericParam::Type(_)))
    {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "JavaSerializable supports only type parameters (no lifetimes or const generics)",
        ));
    }

    let attrs = parse_java_attrs(&input.attrs)?;
    let Some(class) = attrs.class else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing `#[java(class = \"...\")]`",
        ));
    };
    if attrs.rename.is_some() || attrs.transient || attrs.flatten {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`rename`/`transient`/`flatten` are field attributes",
        ));
    }

    let name = &input.ident;
    let descriptor = format!("L{};", class.replace('.', "/"));
    let array_descriptor = format!("[L{};", class.replace('.', "/"));

    if let Data::Enum(data) = &input.data {
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "JavaSerializable generic enums are not supported",
            ));
        }
        return expand_enum(
            name,
            &class,
            attrs.suid.unwrap_or(0),
            &descriptor,
            &array_descriptor,
            data,
        );
    }

    let Some(suid) = attrs.suid else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing `#[java(suid = ...)]`",
        ));
    };

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "JavaSerializable can only be derived for structs or unit-only enums",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "JavaSerializable requires named fields",
        ));
    };

    let mut infos = Vec::new();
    for field in &fields.named {
        infos.push(field_info(field)?);
    }

    let flatten_count = infos.iter().filter(|field| field.flatten).count();
    if flatten_count > 1 {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "at most one `#[java(flatten)]` superclass field is supported",
        ));
    }

    // Java wire order: primitives by name, then object/array fields by name.
    infos.sort_by(|left, right| {
        left.primitive
            .cmp(&right.primitive)
            .reverse()
            .then_with(|| left.java_name.cmp(&right.java_name))
    });

    let schema = infos
        .iter()
        .filter(|field| !field.transient && !field.flatten)
        .map(|field| {
            let java_name = &field.java_name;
            let typecode = &field.typecode;
            let class_name1 = match &field.class_name1 {
                Some(value) => quote! { Some(#value) },
                None => quote! { None },
            };
            quote! {
                jser::JavaField {
                    name: #java_name,
                    typecode: #typecode,
                    class_name1: #class_name1,
                }
            }
        });

    let desc_fields = infos
        .iter()
        .filter(|field| !field.transient && !field.flatten)
        .map(|field| {
            let java_name = &field.java_name;
            let typecode = &field.typecode;
            let class_name1 = match &field.class_name1 {
                Some(value) => quote! { Some(#value.to_owned()) },
                None => quote! { None },
            };
            quote! {
                desc_fields.push(jser::FieldDesc {
                    name: #java_name.to_owned(),
                    typecode: #typecode,
                    class_name1: #class_name1,
                    class_name_handle: None,
                });
            }
        });

    let values = infos
        .iter()
        .filter(|field| !field.transient && !field.flatten)
        .map(|field| {
            let ident = &field.ident;
            let java_name = &field.java_name;
            let array_value = |collection: proc_macro2::TokenStream| {
                let array_descriptor = field
                    .array_descriptor
                    .as_ref()
                    .expect("array fields have a descriptor");
                quote! {
                    {
                        let mut encoded = Vec::new();
                        for value in #collection {
                            encoded.push(jser::IntoJValue::into_jvalue(value, graph)?);
                        }
                        let handle =
                            jser::builtin::new_java_array(graph, #array_descriptor, encoded)?;
                        jser::JValue::Arr(handle)
                    }
                }
            };
            let value = if field.vec {
                array_value(quote! { self.#ident.clone() })
            } else if field.optional_vec {
                let some = array_value(quote! { values });
                quote! {
                    match self.#ident.clone() {
                        Some(values) => #some,
                        None => jser::JValue::Null,
                    }
                }
            } else {
                quote! {
                    jser::IntoJValue::into_jvalue(self.#ident.clone(), graph)?
                }
            };
            quote! { object_fields.push((#java_name.to_owned(), #value)); }
        });

    let initializers = infos.iter().map(|field| {
        let ident = &field.ident;
        if field.transient {
            quote! { #ident: Default::default() }
        } else if field.flatten {
            quote! { #ident: jser::FromJValue::from_jvalue(value, graph)? }
        } else {
            let java_name = &field.java_name;
            quote! {
                #ident: jser::FromJValue::from_jvalue(
                    graph.require_field(value, #java_name)?,
                    graph,
                )?
            }
        }
    });

    let (parent_setup, super_desc) = if let Some(field) = infos.iter().find(|field| field.flatten) {
        let ident = &field.ident;
        (
            quote! {
                let parent_value =
                    jser::IntoJValue::into_jvalue(self.#ident.clone(), graph)?;
                let mut parent_classes = graph
                    .as_object(&parent_value)?
                    .ok_or_else(|| jser::Error::FieldMismatch {
                        message: "flatten field must be a Java object".to_owned(),
                    })?
                    .classes
                    .clone();
                let parent_desc = parent_classes
                    .last()
                    .ok_or_else(|| jser::Error::FieldMismatch {
                        message: "flatten field has no class data".to_owned(),
                    })?
                    .desc;
            },
            quote! { Some(parent_desc) },
        )
    } else {
        (
            quote! { let mut parent_classes = Vec::new(); },
            quote! { None },
        )
    };

    let mut generics = input.generics.clone();
    for param in generics.type_params_mut() {
        param.bounds.push(parse_quote!(jser::JavaSerializable));
        param.bounds.push(parse_quote!(jser::IntoJValue));
        param.bounds.push(parse_quote!(jser::FromJValue));
        param.bounds.push(parse_quote!(Clone));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let output = quote! {
        impl #impl_generics jser::JavaSerializable for #name #ty_generics #where_clause {
            const CLASS: &'static str = #class;
            const SUID: i64 = #suid;
            const DESCRIPTOR: &'static str = #descriptor;
            const ARRAY_DESCRIPTOR: &'static str = #array_descriptor;

            fn schema() -> Vec<jser::JavaField> {
                vec![#(#schema),*]
            }

            fn to_jvalue(&self, graph: &mut jser::Graph) -> jser::Result<jser::JValue> {
                #parent_setup
                let mut desc_fields = Vec::new();
                #(#desc_fields)*
                let desc = jser::ClassDesc {
                    name: Self::CLASS.to_owned(),
                    suid: Self::SUID,
                    flags: 0x02,
                    fields: desc_fields,
                    super_desc: #super_desc,
                    annotations: Vec::new(),
                    proxy_interfaces: None,
                };
                let desc = graph.add_class_desc(desc)?;

                let mut object_fields = Vec::new();
                #(#values)*
                parent_classes.push(jser::ClassData {
                    desc,
                    fields: object_fields,
                    custom: None,
                });
                let object = graph.new_object(parent_classes)?;
                Ok(jser::JValue::Obj(object))
            }

            fn from_jvalue(
                value: &jser::JValue,
                graph: &jser::Graph,
            ) -> jser::Result<Self> {
                <Self as jser::JavaSerializable>::validate_decoded(value, graph)?;
                Ok(Self {
                    #(#initializers),*
                })
            }
        }

        impl #impl_generics jser::FromJValue for #name #ty_generics #where_clause {
            fn from_jvalue(
                value: &jser::JValue,
                graph: &jser::Graph,
            ) -> jser::Result<Self> {
                <Self as jser::JavaSerializable>::from_jvalue(value, graph)
            }
        }

        impl #impl_generics jser::IntoJValue for #name #ty_generics #where_clause {
            fn into_jvalue(self, graph: &mut jser::Graph) -> jser::Result<jser::JValue> {
                <Self as jser::JavaSerializable>::to_jvalue(&self, graph)
            }
        }
    };

    Ok(output)
}

fn expand_enum(
    name: &syn::Ident,
    class: &str,
    suid: i64,
    descriptor: &str,
    array_descriptor: &str,
    data: &syn::DataEnum,
) -> syn::Result<proc_macro2::TokenStream> {
    if suid != 0 {
        return Err(syn::Error::new_spanned(
            name,
            "Java enum serialVersionUID must be 0",
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "a Java enum needs at least one variant",
        ));
    }

    let mut variants = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "Java enum variants must be unit variants",
            ));
        }
        let attrs = parse_java_attrs(&variant.attrs)?;
        if attrs.class.is_some() || attrs.suid.is_some() || attrs.transient || attrs.flatten {
            return Err(syn::Error::new_spanned(
                variant,
                "enum variants only support `rename`",
            ));
        }
        let java_name = attrs
            .rename
            .unwrap_or_else(|| java_enum_constant(&variant.ident));
        variants.push((variant.ident.clone(), java_name));
    }

    let to_arms = variants
        .iter()
        .map(|(ident, name)| quote! { Self::#ident => #name, });
    let from_arms = variants
        .iter()
        .map(|(ident, name)| quote! { #name => Ok(Self::#ident), });

    let output = quote! {
        impl jser::JavaSerializable for #name {
            const CLASS: &'static str = #class;
            const SUID: i64 = 0;
            const DESCRIPTOR: &'static str = #descriptor;
            const ARRAY_DESCRIPTOR: &'static str = #array_descriptor;

            fn schema() -> Vec<jser::JavaField> {
                Vec::new()
            }

            fn to_jvalue(&self, graph: &mut jser::Graph) -> jser::Result<jser::JValue> {
                let constant: &'static str = match self {
                    #( #to_arms )*
                };
                let enum_desc = graph.add_class_desc(jser::ClassDesc {
                    name: "java.lang.Enum".to_owned(),
                    suid: 0,
                    flags: 0x12,
                    fields: Vec::new(),
                    super_desc: None,
                    annotations: Vec::new(),
                    proxy_interfaces: None,
                })?;
                let desc = graph.add_class_desc(jser::ClassDesc {
                    name: Self::CLASS.to_owned(),
                    suid: 0,
                    flags: 0x12,
                    fields: Vec::new(),
                    super_desc: Some(enum_desc),
                    annotations: Vec::new(),
                    proxy_interfaces: None,
                })?;
                let constant = graph.new_string(constant);
                let value = graph.new_enum(desc, constant)?;
                Ok(jser::JValue::Enum(value))
            }

            fn from_jvalue(
                value: &jser::JValue,
                graph: &jser::Graph,
            ) -> jser::Result<Self> {
                <Self as jser::JavaSerializable>::validate_decoded(value, graph)?;
                let constant = graph.as_enum_constant(value)?.ok_or_else(|| {
                    jser::Error::FieldMismatch {
                        message: format!("expected a Java enum for `{}`", Self::CLASS),
                    }
                })?;
                match constant {
                    #( #from_arms )*
                    other => Err(jser::Error::FieldMismatch {
                        message: format!(
                            "unknown enum constant `{other}` for `{}`",
                            Self::CLASS
                        ),
                    }),
                }
            }
        }

        impl jser::FromJValue for #name {
            fn from_jvalue(
                value: &jser::JValue,
                graph: &jser::Graph,
            ) -> jser::Result<Self> {
                <Self as jser::JavaSerializable>::from_jvalue(value, graph)
            }
        }

        impl jser::IntoJValue for #name {
            fn into_jvalue(self, graph: &mut jser::Graph) -> jser::Result<jser::JValue> {
                <Self as jser::JavaSerializable>::to_jvalue(&self, graph)
            }
        }
    };

    Ok(output)
}

fn java_enum_constant(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    let name = name.trim_start_matches("r#");
    let mut constant = String::new();
    let mut previous: Option<char> = None;

    for character in name.chars() {
        if character.is_uppercase() {
            if let Some(previous) = previous {
                if previous.is_lowercase() || previous.is_ascii_digit() {
                    constant.push('_');
                }
            }
        }
        constant.push(character.to_ascii_uppercase());
        previous = Some(character);
    }

    constant
}

fn field_info(field: &Field) -> syn::Result<FieldInfo> {
    let ident = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new_spanned(field, "expected a named field"))?;
    let attrs = parse_java_attrs(&field.attrs)?;
    if attrs.class.is_some() || attrs.suid.is_some() {
        return Err(syn::Error::new_spanned(
            field,
            "`class`/`suid` are struct attributes",
        ));
    }

    let java_name = attrs
        .rename
        .clone()
        .unwrap_or_else(|| ident.to_string().trim_start_matches("r#").to_owned());
    let ty = field_type(&field.ty, &java_name)?;

    if attrs.flatten {
        if attrs.transient || attrs.rename.is_some() {
            return Err(syn::Error::new_spanned(
                field,
                "`flatten` cannot be combined with `transient` or `rename`",
            ));
        }
        if !ty.custom {
            return Err(syn::Error::new_spanned(
                field,
                "`#[java(flatten)]` requires a JavaSerializable field type",
            ));
        }
    }

    Ok(FieldInfo {
        ident,
        java_name,
        typecode: ty.typecode,
        class_name1: ty.class_name1,
        primitive: ty.primitive,
        vec: ty.vec,
        optional_vec: ty.optional_vec,
        array_descriptor: ty.array_descriptor,
        transient: attrs.transient,
        flatten: attrs.flatten,
    })
}

struct TypeInfo {
    typecode: proc_macro2::TokenStream,
    class_name1: Option<proc_macro2::TokenStream>,
    descriptor: String,
    primitive: bool,
    vec: bool,
    optional_vec: bool,
    array_descriptor: Option<proc_macro2::TokenStream>,
    custom: bool,
}

fn field_type(ty: &Type, field_name: &str) -> syn::Result<TypeInfo> {
    let ident = type_ident(ty).ok_or_else(|| {
        syn::Error::new_spanned(
            ty,
            format!("unsupported JavaSerializable field type for `{field_name}`"),
        )
    })?;
    let name = ident.to_string();

    let primitive = |code: &str, typecode: proc_macro2::TokenStream| TypeInfo {
        typecode,
        class_name1: None,
        descriptor: code.to_owned(),
        primitive: true,
        vec: false,
        optional_vec: false,
        array_descriptor: None,
        custom: false,
    };

    match name.as_str() {
        "bool" => Ok(primitive("Z", quote! { jser::FieldType::Boolean })),
        "i8" => Ok(primitive("B", quote! { jser::FieldType::Byte })),
        "char" | "u16" => Ok(primitive("C", quote! { jser::FieldType::Char })),
        "i16" => Ok(primitive("S", quote! { jser::FieldType::Short })),
        "i32" => Ok(primitive("I", quote! { jser::FieldType::Int })),
        "i64" => Ok(primitive("J", quote! { jser::FieldType::Long })),
        "f32" => Ok(primitive("F", quote! { jser::FieldType::Float })),
        "f64" => Ok(primitive("D", quote! { jser::FieldType::Double })),
        "u8" | "u32" | "u64" | "usize" | "i128" | "u128" => Err(syn::Error::new_spanned(
            ty,
            format!(
                "unsupported integer field type `{name}`; use i8/i16/i32/i64 (Java `byte` is `i8`)"
            ),
        )),
        "String" => Ok(object("Ljava/lang/String;")),
        "HashMap" => Ok(object("Ljava/util/HashMap;")),
        "HashSet" => Ok(object("Ljava/util/HashSet;")),
        "Option" => {
            let inner = generic_arg(ty, 0, "Option")?;
            let inner = field_type(inner, field_name)?;
            if inner.vec {
                return Ok(TypeInfo {
                    typecode: quote! { jser::FieldType::Object },
                    class_name1: inner.class_name1,
                    descriptor: inner.descriptor,
                    primitive: false,
                    vec: false,
                    optional_vec: true,
                    array_descriptor: inner.array_descriptor,
                    custom: false,
                });
            }
            if inner.primitive {
                return Err(syn::Error::new_spanned(
                    ty,
                    "Option<T> requires an object type (String, custom class, HashMap, HashSet)",
                ));
            }
            if inner.custom {
                Ok(TypeInfo {
                    typecode: quote! { jser::FieldType::Object },
                    class_name1: inner.class_name1,
                    descriptor: String::new(),
                    primitive: false,
                    vec: false,
                    optional_vec: false,
                    array_descriptor: inner.array_descriptor,
                    custom: true,
                })
            } else {
                Ok(object_owned(inner.descriptor))
            }
        }
        "Vec" => {
            let inner_ty = generic_arg(ty, 0, "Vec")?.clone();
            let inner = field_type(&inner_ty, field_name)?;
            if inner.vec {
                return Err(syn::Error::new_spanned(
                    ty,
                    "nested Vec fields are not supported",
                ));
            }
            if inner.custom {
                let array_descriptor = inner
                    .array_descriptor
                    .expect("custom Vec elements have an array descriptor");
                return Ok(TypeInfo {
                    typecode: quote! { jser::FieldType::Array },
                    class_name1: Some(array_descriptor.clone()),
                    descriptor: String::new(),
                    primitive: false,
                    vec: true,
                    optional_vec: false,
                    array_descriptor: Some(array_descriptor),
                    custom: false,
                });
            }
            if inner.class_name1.is_some() && !inner.descriptor.starts_with('L') {
                return Err(syn::Error::new_spanned(
                    ty,
                    "nested Vec fields are not supported",
                ));
            }
            let array_class = array_class_name(&inner.descriptor);
            if !matches!(
                array_class.as_str(),
                "[Z" | "[B"
                    | "[C"
                    | "[S"
                    | "[I"
                    | "[J"
                    | "[F"
                    | "[D"
                    | "[Ljava.lang.String;"
                    | "[Ljava.lang.Object;"
            ) {
                return Err(syn::Error::new_spanned(
                    ty,
                    "Vec<T> supports only primitive/String elements",
                ));
            }
            Ok(TypeInfo {
                typecode: quote! { jser::FieldType::Array },
                class_name1: Some(quote! { #array_class }),
                descriptor: array_class.clone(),
                primitive: false,
                vec: true,
                optional_vec: false,
                array_descriptor: Some(quote! { #array_class }),
                custom: false,
            })
        }
        _ => Ok(TypeInfo {
            typecode: quote! { jser::FieldType::Object },
            class_name1: Some(quote! { <#ty as jser::JavaSerializable>::DESCRIPTOR }),
            descriptor: String::new(),
            primitive: false,
            vec: false,
            optional_vec: false,
            array_descriptor: Some(quote! { <#ty as jser::JavaSerializable>::ARRAY_DESCRIPTOR }),
            custom: true,
        }),
    }
}

fn object(descriptor: &str) -> TypeInfo {
    object_owned(descriptor.to_owned())
}

fn object_owned(descriptor: String) -> TypeInfo {
    TypeInfo {
        typecode: quote! { jser::FieldType::Object },
        class_name1: Some(quote! { #descriptor }),
        descriptor,
        primitive: false,
        vec: false,
        optional_vec: false,
        array_descriptor: None,
        custom: false,
    }
}

fn type_ident(ty: &Type) -> Option<&syn::Ident> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    path.path.segments.last().map(|segment| &segment.ident)
}

fn generic_arg<'a>(ty: &'a Type, index: usize, expected: &str) -> syn::Result<&'a Type> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(ty, "expected a generic type"));
    };
    let segment = path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "missing type"))?;
    if segment.ident != expected {
        return Err(syn::Error::new_spanned(ty, format!("expected {expected}")));
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(ty, "expected generic arguments"));
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .nth(index)
        .ok_or_else(|| syn::Error::new_spanned(ty, "missing generic argument"))
}

fn array_class_name(element_descriptor: &str) -> String {
    if let Some(class) = element_descriptor
        .strip_prefix('L')
        .and_then(|value| value.strip_suffix(';'))
    {
        format!("[L{};", class.replace('/', "."))
    } else {
        format!("[{element_descriptor}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn missing_class_attribute_is_an_error() {
        let input: DeriveInput = parse_quote! {
            struct Foo {
                x: i32,
            }
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("missing `#[java(class"));
    }

    #[test]
    fn unsupported_field_type_is_an_error() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo {
                value: u8,
            }
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("unsupported integer field type"));
        assert!(error.to_string().contains("u8"));
    }

    #[test]
    fn combination_fields_are_supported() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo {
                maybe_values: Option<Vec<i32>>,
                maybe_map: Option<HashMap<String, i32>>,
                options: Vec<Option<String>>,
            }
        };
        assert!(expand(input).is_ok());
    }

    #[test]
    fn generic_structs_are_supported() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo<T> {
                value: T,
            }
        };
        assert!(expand(input).is_ok());
    }

    #[test]
    fn vec_of_custom_types_is_supported() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo {
                values: Vec<Bar>,
            }
        };
        assert!(expand(input).is_ok());
    }

    #[test]
    fn flatten_requires_a_custom_type() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo {
                #[java(flatten)]
                value: i32,
            }
        };
        let error = expand(input).unwrap_err();
        assert!(error
            .to_string()
            .contains("requires a JavaSerializable field type"));
    }

    #[test]
    fn enum_variants_must_be_unit() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo")]
            enum Foo {
                A(i32),
            }
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("unit variants"));
    }

    #[test]
    fn enum_suid_must_be_zero() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            enum Foo {
                A,
            }
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("must be 0"));
    }

    #[test]
    fn enums_default_to_suid_zero() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo")]
            enum Foo {
                A,
            }
        };
        assert!(expand(input).is_ok());
    }

    #[test]
    fn tuple_structs_are_an_error() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo(i32);
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("named fields"));
    }
}
