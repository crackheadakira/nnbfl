use std::collections::HashMap;

use crate::core::{VersionFormat, context::ContextStore};

pub struct Writer {
    pub buffer: Vec<u8>,
    pub breadcrumbs: Vec<(usize, String)>,
    pub version: VersionFormat,
    pub section_start: Option<usize>,
    pub string_pool: Option<StringPool>,
    pub contexts: ContextStore,
}

#[derive(Debug, Clone, Copy)]
pub struct Placeholder32(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct Placeholder16(pub usize);

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct StringPool {
    bytes: Vec<u8>,
    offsets: HashMap<String, u32>,
    start: Option<usize>,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(65536),
            breadcrumbs: Vec::new(),
            version: VersionFormat::default(),
            section_start: None,
            string_pool: None,
            contexts: ContextStore::default(),
        }
    }

    pub fn ctx_section_start<T>(&self) -> usize {
        self.section_start.unwrap_or_else(|| {
            let full_name = std::any::type_name::<T>();
            let short_name = full_name.split("::").last().unwrap_or(full_name);
            
            panic!("CRITICAL: Failed to serialize {short_name} because the parent block did not establish a section_start context.");
        })
    }

    pub fn mark(&mut self, name: &str) {
        self.breadcrumbs.push((self.pos(), name.to_owned()));
    }

    pub fn pos(&self) -> usize {
        self.buffer.len()
    }

    pub fn write_u8(&mut self, val: u8) {
        self.buffer.push(val);
    }

    pub fn write_i16(&mut self, val: i16) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u16(&mut self, val: u16) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u32(&mut self, val: u32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u64(&mut self, val: u64) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_i32(&mut self, val: i32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_f32(&mut self, val: f32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn write_fixed_string(&mut self, s: &str, len: usize) {
        let bytes = s.as_bytes();
        let write_len = bytes.len().min(len);
        self.buffer.extend_from_slice(&bytes[..write_len]);

        if write_len < len {
            self.buffer.resize(self.buffer.len() + (len - write_len), 0);
        }
    }

    pub fn write_null_terminated_string(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        self.buffer.push(0);
    }

    pub fn write_placeholder_u16(&mut self) -> Placeholder16 {
        let pos = self.pos();
        self.write_u16(0);
        Placeholder16(pos)
    }

    pub fn write_placeholder_u32(&mut self) -> Placeholder32 {
        let pos = self.pos();
        self.write_u32(0);
        Placeholder32(pos)
    }

    pub fn patch_u16(&mut self, pos: Placeholder16, val: u16) {
        let bytes = val.to_le_bytes();
        self.buffer[pos.0..pos.0 + 2].copy_from_slice(&bytes);
    }

    pub fn patch_u32(&mut self, pos: Placeholder32, val: u32) {
        let bytes = val.to_le_bytes();
        self.buffer[pos.0..pos.0 + 4].copy_from_slice(&bytes);
    }

    pub fn align(&mut self, alignment: usize) {
        let remainder = self.pos() % alignment;
        if remainder != 0 {
            let padding = alignment - remainder;
            self.buffer.resize(self.buffer.len() + padding, 0);
        }
    }

    pub fn intern_string(&mut self, value: &str) -> u32 {
        let pool = self.string_pool.get_or_insert_with(StringPool::default);

        if let Some(&offset) = pool.offsets.get(value) {
            return offset
        }

        let offset = u32::try_from(pool.bytes.len()).expect("String pool exceeds u32 offsets");
        
        pool.bytes.extend_from_slice(value.as_bytes());
        pool.bytes.push(0);
        pool.offsets.insert(value.to_owned(), offset);

        offset
    }

    pub fn write_string_offset(&mut self, value: &str) {
        let offset = self.intern_string(value);
        self.write_u32(offset);
    }

    pub fn write_string_pool(&mut self) -> usize {
        let start = self.pos();
        let pool = self.string_pool.get_or_insert_with(StringPool::default);

        self.buffer.extend_from_slice(&pool.bytes);
        pool.start = Some(start);

        start
    }

    pub fn context<T: std::any::Any>(&self) -> &T {
        self.contexts.get::<T>().unwrap_or_else(|| {
            panic!(
                "Missing serialization context '{}' at byte offset 0x{:X}",
                std::any::type_name::<T>(),
                self.pos(),
            )
        })
    }

    pub fn with_context<T: std::any::Any + Send + Sync, R>(&mut self, value: T, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.contexts.replace(value);

        let result = f(self);

        self.contexts.restore::<T>(previous);

        result
    }
}
