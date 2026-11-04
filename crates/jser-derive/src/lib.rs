//! `#[derive(JavaSerializable)]` for `jser`.
//!
//! Attributes:
//! - `#[java(class = "com.example.Foo", suid = 1)]` on the struct (both required)
//! - `#[java(rename = "javaField")]` on a field
//! - `#[java(transient)]` on a field (not serialized; defaults on decode)
//!
//! Supported field types: Java primitives, `String`, `Option<T>`, `Vec<T>`
//! (arrays), `HashMap<K, V>` and `HashSet<T>`. Derived structs must implement
//! `Clone` because the encoder converts fields by value.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, GenericArgument, LitInt,
    LitStr, PathArguments, Type,
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
    array_class: Option<String>,
    vec_element: Option<syn::Type>,
    transient: bool,
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "JavaSerializable does not support generic structs",
        ));
    }

    let attrs = parse_java_attrs(&input.attrs)?;
    let Some(class) = attrs.class else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing `#[java(class = \"...\")]`",
        ));
    };
    let Some(suid) = attrs.suid else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing `#[java(suid = ...)]`",
        ));
    };
    if attrs.rename.is_some() || attrs.transient {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`rename`/`transient` are field attributes",
        ));
    }

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "JavaSerializable can only be derived for structs",
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

    // Java wire order: primitives by name, then object/array fields by name.
    infos.sort_by(|left, right| {
        left.primitive
            .cmp(&right.primitive)
            .reverse()
            .then_with(|| left.java_name.cmp(&right.java_name))
    });

    let name = &input.ident;
    let descriptor = format!("L{};", class.replace('.', "/"));
    let array_descriptor = format!("[L{};", class.replace('.', "/"));
    let schema = infos.iter().filter(|field| !field.transient).map(|field| {
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

    let desc_fields = infos.iter().filter(|field| !field.transient).map(|field| {
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

    let values = infos.iter().filter(|field| !field.transient).map(|field| {
        let ident = &field.ident;
        let java_name = &field.java_name;
        let value = if field.vec {
            if let Some(element_ty) = &field.vec_element {
                quote! {
                    {
                        let mut values = Vec::new();
                        for value in self.#ident.clone() {
                            values.push(jser::IntoJValue::into_jvalue(value, graph)?);
                        }
                        let class_name =
                            format!("[L{};", <#element_ty as jser::JavaSerializable>::CLASS);
                        let handle =
                            jser::builtin::new_java_array(graph, &class_name, values)?;
                        jser::JValue::Arr(handle)
                    }
                }
            } else {
                let array_class = field
                    .array_class
                    .as_deref()
                    .expect("Vec fields have an array class");
                quote! {
                    {
                        let mut values = Vec::new();
                        for value in self.#ident.clone() {
                            values.push(jser::IntoJValue::into_jvalue(value, graph)?);
                        }
                        let handle = jser::builtin::new_java_array(graph, #array_class, values)?;
                        jser::JValue::Arr(handle)
                    }
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

    let output = quote! {
        impl jser::JavaSerializable for #name {
            const CLASS: &'static str = #class;
            const SUID: i64 = #suid;
            const DESCRIPTOR: &'static str = #descriptor;
            const ARRAY_DESCRIPTOR: &'static str = #array_descriptor;

            fn schema() -> &'static [jser::JavaField] {
                static FIELDS: &[jser::JavaField] = &[#(#schema),*];
                FIELDS
            }

            fn to_jvalue(&self, graph: &mut jser::Graph) -> jser::Result<jser::JValue> {
                let mut desc_fields = Vec::new();
                #(#desc_fields)*
                let desc = jser::ClassDesc {
                    name: Self::CLASS.to_owned(),
                    suid: Self::SUID,
                    flags: 0x02,
                    fields: desc_fields,
                    super_desc: None,
                    annotations: Vec::new(),
                };
                let desc = graph.add_class_desc(desc)?;

                let mut object_fields = Vec::new();
                #(#values)*
                let object = graph.new_object(vec![jser::ClassData {
                    desc,
                    fields: object_fields,
                    custom: None,
                }])?;
                Ok(jser::JValue::Obj(object))
            }

            fn from_jvalue(
                value: &jser::JValue,
                graph: &jser::Graph,
            ) -> jser::Result<Self> {
                Ok(Self {
                    #(#initializers),*
                })
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
        .unwrap_or_else(|| ident.to_string().trim_start_matches("r#").to_owned());
    let ty = field_type(&field.ty, &java_name)?;

    Ok(FieldInfo {
        ident,
        java_name,
        typecode: ty.typecode,
        class_name1: ty.class_name1,
        primitive: ty.primitive,
        vec: ty.vec,
        array_class: ty.array_class,
        vec_element: ty.vec_element,
        transient: attrs.transient,
    })
}

struct TypeInfo {
    typecode: proc_macro2::TokenStream,
    class_name1: Option<proc_macro2::TokenStream>,
    descriptor: String,
    primitive: bool,
    vec: bool,
    array_class: Option<String>,
    vec_element: Option<syn::Type>,
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
        array_class: None,
        vec_element: None,
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
            if inner.primitive || inner.vec {
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
                    array_class: None,
                    vec_element: None,
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
                return Ok(TypeInfo {
                    typecode: quote! { jser::FieldType::Array },
                    class_name1: Some(
                        quote! { <#inner_ty as jser::JavaSerializable>::ARRAY_DESCRIPTOR },
                    ),
                    descriptor: String::new(),
                    primitive: false,
                    vec: true,
                    array_class: None,
                    vec_element: Some(inner_ty),
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
                array_class: Some(array_class),
                vec_element: None,
                custom: false,
            })
        }
        _ => Ok(TypeInfo {
            typecode: quote! { jser::FieldType::Object },
            class_name1: Some(quote! { <#ty as jser::JavaSerializable>::DESCRIPTOR }),
            descriptor: String::new(),
            primitive: false,
            vec: false,
            array_class: None,
            vec_element: None,
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
        array_class: None,
        vec_element: None,
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
    fn tuple_structs_are_an_error() {
        let input: DeriveInput = parse_quote! {
            #[java(class = "Foo", suid = 1)]
            struct Foo(i32);
        };
        let error = expand(input).unwrap_err();
        assert!(error.to_string().contains("named fields"));
    }
}
