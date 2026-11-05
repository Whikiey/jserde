//! Port of `ObjectStreamClass.computeDefaultSUID` (JDK 17).
//!
//! The signature is byte-for-byte what `DataOutputStream` would write:
//! class name, class modifiers, sorted interfaces, sorted fields, optional
//! `<clinit>`, sorted constructors and sorted non-private methods. SHA-1 is
//! then folded byte-by-byte from the **end** of the first 8 digest bytes.

use sha1::{Digest, Sha1};

use crate::error::{Error, Result};
use crate::wire::mutf8;

const PUBLIC: u32 = 0x0001;
const PRIVATE: u32 = 0x0002;
const PROTECTED: u32 = 0x0004;
const STATIC: u32 = 0x0008;
const FINAL: u32 = 0x0010;
const SYNCHRONIZED: u32 = 0x0020;
const VOLATILE: u32 = 0x0040;
const TRANSIENT: u32 = 0x0080;
const NATIVE: u32 = 0x0100;
const INTERFACE: u32 = 0x0200;
const ABSTRACT: u32 = 0x0400;
const STRICT: u32 = 0x0800;

const CLASS_MASK: u32 = PUBLIC | FINAL | INTERFACE | ABSTRACT;
const FIELD_MASK: u32 = PUBLIC | PRIVATE | PROTECTED | STATIC | FINAL | VOLATILE | TRANSIENT;
const MEMBER_MASK: u32 =
    PUBLIC | PRIVATE | PROTECTED | STATIC | FINAL | SYNCHRONIZED | NATIVE | ABSTRACT | STRICT;

/// One serializable field in a class signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSignature {
    pub name: String,
    pub modifiers: u32,
    /// JVM field descriptor (`I`, `Ljava/lang/String;`, `[I`).
    pub descriptor: String,
}

/// One constructor or method in a class signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSignature {
    pub name: String,
    pub modifiers: u32,
    /// JVM method descriptor (`()V`, `(ILjava/lang/String;)V`).
    pub descriptor: String,
}

/// Metadata needed to reproduce the JDK default-SUID algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSignature {
    pub class_name: String,
    /// Raw `Class.getModifiers()` value.
    pub modifiers: u32,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldSignature>,
    pub has_static_initializer: bool,
    pub constructors: Vec<MemberSignature>,
    pub methods: Vec<MemberSignature>,
}

/// Compute the JDK default `serialVersionUID` for a class signature.
pub fn compute_default_suid(signature: &ClassSignature) -> Result<i64> {
    let mut bytes = Vec::new();

    write_utf(&mut bytes, &signature.class_name)?;

    let mut class_modifiers = signature.modifiers & CLASS_MASK;
    if class_modifiers & INTERFACE != 0 {
        // JDK compensates for a javac bug: an interface is marked abstract only
        // when it declares methods.
        class_modifiers = if signature.methods.is_empty() {
            class_modifiers & !ABSTRACT
        } else {
            class_modifiers | ABSTRACT
        };
    }
    bytes.extend_from_slice(&class_modifiers.to_be_bytes());

    let mut interfaces = signature.interfaces.clone();
    interfaces.sort();
    for interface in &interfaces {
        write_utf(&mut bytes, interface)?;
    }

    let mut fields: Vec<&FieldSignature> = signature.fields.iter().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    for field in fields {
        let modifiers = field.modifiers & FIELD_MASK;
        // Skip private static/transient fields (e.g. serialVersionUID).
        if modifiers & PRIVATE == 0 || modifiers & (STATIC | TRANSIENT) == 0 {
            write_utf(&mut bytes, &field.name)?;
            bytes.extend_from_slice(&modifiers.to_be_bytes());
            write_utf(&mut bytes, &field.descriptor)?;
        }
    }

    if signature.has_static_initializer {
        write_utf(&mut bytes, "<clinit>")?;
        bytes.extend_from_slice(&STATIC.to_be_bytes());
        write_utf(&mut bytes, "()V")?;
    }

    let mut constructors: Vec<&MemberSignature> = signature.constructors.iter().collect();
    constructors.sort_by(|left, right| left.descriptor.cmp(&right.descriptor));
    for constructor in constructors {
        let modifiers = constructor.modifiers & MEMBER_MASK;
        if modifiers & PRIVATE == 0 {
            write_utf(&mut bytes, "<init>")?;
            bytes.extend_from_slice(&modifiers.to_be_bytes());
            write_utf(&mut bytes, &constructor.descriptor.replace('/', "."))?;
        }
    }

    let mut methods: Vec<&MemberSignature> = signature.methods.iter().collect();
    methods.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.descriptor.cmp(&right.descriptor))
    });
    for method in methods {
        let modifiers = method.modifiers & MEMBER_MASK;
        if modifiers & PRIVATE == 0 {
            write_utf(&mut bytes, &method.name)?;
            bytes.extend_from_slice(&modifiers.to_be_bytes());
            write_utf(&mut bytes, &method.descriptor.replace('/', "."))?;
        }
    }

    let digest = Sha1::digest(&bytes);
    let mut hash = 0i64;
    for byte in digest.iter().take(8).rev() {
        hash = (hash << 8) | i64::from(*byte);
    }
    Ok(hash)
}

/// Compute the default SUID of an array class (`[I`, `[Ljava.lang.String;`).
///
/// Array classes report `PUBLIC | FINAL | ABSTRACT` and declare no interfaces,
/// fields, constructors or methods (the JDK skips the interface list for
/// arrays), so the normal default-SUID algorithm applies. This reproduces the
/// JDK values for primitive and `String` arrays and extends to custom element
/// classes (P9-14).
pub fn array_class_suid(class_name: &str) -> Result<i64> {
    if !is_array_class_name(class_name) {
        return Err(Error::InvalidSchema(format!(
            "`{class_name}` is not an array class"
        )));
    }

    compute_default_suid(&ClassSignature {
        class_name: class_name.to_owned(),
        modifiers: PUBLIC | FINAL | ABSTRACT,
        interfaces: Vec::new(),
        fields: Vec::new(),
        has_static_initializer: false,
        constructors: Vec::new(),
        methods: Vec::new(),
    })
}

fn is_array_class_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('[') else {
        return false;
    };
    if rest.len() == 1 {
        return b"BCDFIJSZ".contains(&rest.as_bytes()[0]);
    }
    if let Some(element) = rest.strip_prefix('L') {
        return element.len() > 1 && element.ends_with(';');
    }
    rest.starts_with('[')
}

fn write_utf(bytes: &mut Vec<u8>, value: &str) -> Result<()> {
    let encoded = mutf8::encode(value);
    let length = u16::try_from(encoded.len()).map_err(|_| {
        Error::InvalidSchema(format!("class signature string is too long: `{value}`"))
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(())
}

/// Parse the metadata needed for SUID computation from a `.class` file.
///
/// This is intentionally a minimal reader: constant-pool entries and member
/// attributes are skipped once their names/descriptors are known.
pub fn parse_class_file(bytes: &[u8]) -> Result<ClassSignature> {
    ClassReader::new(bytes).parse()
}

#[derive(Clone)]
enum Constant {
    Empty,
    Utf8(String),
    Class(u16),
    Other,
}

struct ClassReader<'a> {
    bytes: &'a [u8],
    position: usize,
    constants: Vec<Constant>,
}

impl<'a> ClassReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            constants: Vec::new(),
        }
    }

    fn parse(mut self) -> Result<ClassSignature> {
        if self.read_u32()? != 0xCAFE_BABE {
            return Err(class_error("bad magic"));
        }
        let _minor = self.read_u16()?;
        let _major = self.read_u16()?;
        self.read_constant_pool()?;

        let modifiers = u32::from(self.read_u16()?);
        let this_class = self.read_u16()?;
        let _super_class = self.read_u16()?;

        let interface_count = usize::from(self.read_u16()?);
        let mut interfaces = Vec::with_capacity(interface_count);
        for _ in 0..interface_count {
            let index = self.read_u16()?;
            interfaces.push(self.class_name(index)?);
        }

        let field_count = usize::from(self.read_u16()?);
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            let modifiers = u32::from(self.read_u16()?);
            let name_index = self.read_u16()?;
            let descriptor_index = self.read_u16()?;
            let name = self.utf8(name_index)?;
            let descriptor = self.utf8(descriptor_index)?;
            self.skip_attributes()?;
            fields.push(FieldSignature {
                name,
                modifiers,
                descriptor,
            });
        }

        let method_count = usize::from(self.read_u16()?);
        let mut constructors = Vec::new();
        let mut methods = Vec::new();
        let mut has_static_initializer = false;
        for _ in 0..method_count {
            let modifiers = u32::from(self.read_u16()?);
            let name_index = self.read_u16()?;
            let descriptor_index = self.read_u16()?;
            let name = self.utf8(name_index)?;
            let descriptor = self.utf8(descriptor_index)?;
            self.skip_attributes()?;

            let member = MemberSignature {
                name: name.clone(),
                modifiers,
                descriptor,
            };
            match name.as_str() {
                "<clinit>" => has_static_initializer = true,
                "<init>" => constructors.push(member),
                _ => methods.push(member),
            }
        }
        self.skip_attributes()?;

        Ok(ClassSignature {
            class_name: self.class_name(this_class)?,
            modifiers,
            interfaces,
            fields,
            has_static_initializer,
            constructors,
            methods,
        })
    }

    fn read_constant_pool(&mut self) -> Result<()> {
        let count = usize::from(self.read_u16()?);
        self.constants = vec![Constant::Empty; count];
        let mut index = 1;

        while index < count {
            let tag = self.read_u8()?;
            self.constants[index] = match tag {
                1 => {
                    let length = usize::from(self.read_u16()?);
                    let bytes = self.read_bytes(length)?;
                    Constant::Utf8(
                        crate::wire::mutf8::decode(bytes)
                            .map_err(|_| class_error("malformed constant-pool UTF-8"))?,
                    )
                }
                7 => Constant::Class(self.read_u16()?),
                8 | 16 | 19 | 20 => {
                    let _ = self.read_u16()?;
                    Constant::Other
                }
                15 => {
                    self.read_bytes(3)?;
                    Constant::Other
                }
                3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => {
                    self.read_bytes(4)?;
                    Constant::Other
                }
                5 | 6 => {
                    self.read_bytes(8)?;
                    index += 1;
                    Constant::Other
                }
                _ => return Err(class_error(format!("unknown constant-pool tag {tag}"))),
            };
            index += 1;
        }
        Ok(())
    }

    fn class_name(&self, index: u16) -> Result<String> {
        match self.constants.get(usize::from(index)) {
            Some(Constant::Class(name_index)) => Ok(self.utf8(*name_index)?.replace('/', ".")),
            _ => Err(class_error(format!("invalid class constant #{index}"))),
        }
    }

    fn utf8(&self, index: u16) -> Result<String> {
        match self.constants.get(usize::from(index)) {
            Some(Constant::Utf8(value)) => Ok(value.clone()),
            _ => Err(class_error(format!("invalid UTF-8 constant #{index}"))),
        }
    }

    fn skip_attributes(&mut self) -> Result<()> {
        let count = usize::from(self.read_u16()?);
        for _ in 0..count {
            let _name = self.read_u16()?;
            let length = usize::try_from(self.read_u32()?)
                .map_err(|_| class_error("attribute length does not fit in usize"))?;
            self.read_bytes(length)?;
        }
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| class_error("offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| class_error("truncated class file"))?;
        self.position = end;
        Ok(bytes)
    }
}

fn class_error(message: impl Into<String>) -> Error {
    Error::FieldMismatch {
        message: format!("class file: {}", message.into()),
    }
}
