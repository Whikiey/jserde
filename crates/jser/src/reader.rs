//! Top-level stream reader for the P1 codec subset.

use crate::error::{Error, Result};
use crate::value::{BlockData, Graph, JValue};
use crate::wire::handle::WireHandles;
use crate::wire::mutf8;
use crate::wire::reader::StreamReader;
use crate::wire::{
    STREAM_MAGIC, STREAM_VERSION, TC_BLOCKDATA, TC_BLOCKDATALONG, TC_ENDBLOCKDATA, TC_LONGSTRING,
    TC_NULL, TC_REFERENCE, TC_RESET, TC_STRING,
};

/// Streaming decoder. Owns the arena while values are read.
#[derive(Debug)]
pub struct Reader<'a> {
    input: StreamReader<'a>,
    graph: Graph,
    handles: WireHandles,
}

impl<'a> Reader<'a> {
    /// Validate the stream header and create a reader.
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        let mut input = StreamReader::new(bytes);

        let magic = input.read_u16()?;
        if magic != STREAM_MAGIC {
            return Err(Error::BadMagic { found: magic });
        }

        let version = input.read_u16()?;
        if version != STREAM_VERSION {
            return Err(Error::UnsupportedVersion { found: version });
        }

        Ok(Self {
            input,
            graph: Graph::default(),
            handles: WireHandles::new(),
        })
    }

    /// Read the next top-level element, or `None` at end of stream.
    pub fn read_next(&mut self) -> Result<Option<JValue>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        let tag = self.input.read_u8()?;
        let value = self.read_value(tag)?;
        self.graph.push_item(value.clone());
        Ok(Some(value))
    }

    /// Arena accumulated so far.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Consume the reader and return the accumulated arena.
    pub fn into_graph(self) -> Graph {
        self.graph
    }

    fn read_value(&mut self, tag: u8) -> Result<JValue> {
        match tag {
            TC_NULL => Ok(JValue::Null),
            TC_STRING => self.read_string(false),
            TC_LONGSTRING => self.read_string(true),
            TC_REFERENCE => self.read_reference(),
            TC_BLOCKDATA => self.read_block(false),
            TC_BLOCKDATALONG => self.read_block(true),
            TC_RESET => {
                self.handles.reset();
                Ok(JValue::Reset)
            }
            TC_ENDBLOCKDATA => Err(Error::BadTag(tag)),
            _ => Err(Error::BadTag(tag)),
        }
    }

    fn read_string(&mut self, long: bool) -> Result<JValue> {
        let length = if long {
            let value = self.input.read_u64()?;
            usize::try_from(value).map_err(|_| Error::InvalidLength {
                kind: "String",
                value,
            })?
        } else {
            usize::from(self.input.read_u16()?)
        };

        let offset = self.input.position();
        let bytes = self.input.read_exact(length)?;
        let value = mutf8::decode_at(bytes, offset)?;

        let handle = self.graph.insert_str(value, long);
        self.handles.assign(handle);
        Ok(JValue::Str(handle))
    }

    fn read_reference(&mut self) -> Result<JValue> {
        let wire = self.input.read_u32()?;
        let handle = self.handles.get(wire).ok_or(Error::BadHandle(wire))?;

        // P1 only allocates string slots; P2 extends this to arrays/objects/enums.
        Ok(JValue::Str(handle))
    }

    fn read_block(&mut self, long: bool) -> Result<JValue> {
        let length = if long {
            let value = self.input.read_u32()?;
            usize::try_from(value).map_err(|_| Error::InvalidLength {
                kind: "block",
                value: u64::from(value),
            })?
        } else {
            usize::from(self.input.read_u8()?)
        };

        let bytes = self.input.read_exact(length)?.to_vec();
        Ok(JValue::Block(BlockData { bytes, long }))
    }
}

/// Decode every top-level element into one arena.
pub fn decode_stream(bytes: &[u8]) -> Result<Graph> {
    let mut reader = Reader::new(bytes)?;
    while reader.read_next()?.is_some() {}
    Ok(reader.into_graph())
}

/// Decode a stream that contains exactly one non-reset element.
pub fn decode(bytes: &[u8]) -> Result<Graph> {
    let graph = decode_stream(bytes)?;
    let items = graph.items();

    if items.len() != 1 || matches!(&items[0], JValue::Reset) {
        return Err(Error::FieldMismatch {
            message: "decode expects exactly one value; use decode_stream".to_owned(),
        });
    }

    Ok(graph)
}
