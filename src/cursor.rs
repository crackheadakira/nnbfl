pub struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    fn read<T: Copy>(&mut self) -> T {
        let size = std::mem::size_of::<T>();
        let end = self.pos + size;
        let bytes = &self.data[self.pos..end];
        self.pos = end;

        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const T) }
    }

    pub fn read_u32(&mut self) -> u32 {
        u32::from_le(self.read::<u32>())
    }

    pub fn read_i32(&mut self) -> i32 {
        i32::from_le(self.read::<i32>())
    }

    pub fn read_u16(&mut self) -> u16 {
        u16::from_le(self.read::<u16>())
    }

    pub fn read_u8(&mut self) -> u8 {
        u8::from_le(self.read::<u8>())
    }

    pub fn read_f32(&mut self) -> f32 {
        let bytes = self.read_bytes(4);
        let arr: [u8; 4] = bytes.try_into().unwrap();
        f32::from_le_bytes(arr)
    }

    pub fn read_string(&mut self, len: usize) -> String {
        let bytes = self.read_bytes(len);
        String::from_utf8_lossy(bytes).into_owned()
    }

    pub fn read_fixed_string(&mut self, len: usize) -> String {
        let bytes = self.read_bytes(len);
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    pub fn read_null_terminated_string(&mut self) -> String {
        let start = self.pos;
        let mut end = start;

        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }

        let bytes = &self.data[start..end];

        self.pos = if end < self.data.len() { end + 1 } else { end };

        String::from_utf8_lossy(bytes).into_owned()
    }

    pub fn read_bytes(&mut self, len: usize) -> &[u8] {
        let start = self.pos;
        self.pos += len;
        &self.data[start..start + len]
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn seek_relative(&mut self, pos: usize) {
        self.pos += pos;
    }
}

pub struct Writer {
    pub buffer: Vec<u8>,
    pub breadcrumbs: Vec<(usize, String)>,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            breadcrumbs: Vec::new(),
        }
    }

    pub fn mark(&mut self, name: &str) {
        self.breadcrumbs.push((self.pos(), name.to_string()));
    }

    pub fn pos(&self) -> usize {
        self.buffer.len()
    }

    pub fn write_u8(&mut self, val: u8) {
        self.buffer.push(val);
    }

    pub fn write_u16(&mut self, val: u16) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u32(&mut self, val: u32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_i32(&mut self, val: i32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_f32(&mut self, val: f32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_fixed_string(&mut self, s: &str, len: usize) {
        let bytes = s.as_bytes();
        let write_len = bytes.len().min(len);
        self.buffer.extend_from_slice(&bytes[..write_len]);

        for _ in write_len..len {
            self.buffer.push(0);
        }
    }

    pub fn write_null_terminated_string(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        self.buffer.push(0);
    }

    pub fn write_placeholder_u32(&mut self) -> usize {
        let pos = self.pos();
        self.write_u32(0);
        pos
    }

    pub fn patch_u32(&mut self, pos: usize, val: u32) {
        let bytes = val.to_le_bytes();
        self.buffer[pos..pos + 4].copy_from_slice(&bytes);
    }

    pub fn align(&mut self, alignment: usize) {
        let remainder = self.pos() % alignment;
        if remainder != 0 {
            let padding = alignment - remainder;
            for _ in 0..padding {
                self.write_u8(0);
            }
        }
    }
}
