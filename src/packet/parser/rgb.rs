use std::ptr::read_unaligned;

use crate::packet::RgbPacket;

#[derive(Debug)]
#[repr(C, packed)]
struct RawRgbPacketHeader {
    sequence: u32,
    // is 'BBBB' equal 0x42424242
    magic_header: u32,
}

// starting from JPEG EOI: 0xff 0xd9
// char pad_0xa5[]; //0-3 bytes alignment of 0xa5
// char filler[filler_length] = "ZZZZ...";
#[derive(Debug)]
#[repr(C, packed)]
struct RawRgbPacketFooter {
    // is '9999' equal 0x39393939
    magic_header: u32,
    sequence: u32,
    filler_length: u32,
    // seems 0 always
    _unknown0: u32,
    // seems 0 always
    _unknown1: u32,
    timestamp: u32,
    // ? ranges from 0.5 to about 60.0 with powerfull light at camera or totally covered
    exposure: f32,
    // ? ranges from 1.0 when camera is clear to 1.5 when camera is covered.
    gain: f32,
    // is 'BBBB' equal 0x42424242
    magic_footer: u32,
    packet_size: u32,
    // ranges from 1.0f to about 6.4 when camera is fully covered
    gamma: f32,
    // seems 0 always
    _unknown2: [u32; 3],
}

pub struct RgbStreamParser {
    memory: Vec<u8>,
}

impl RgbStreamParser {
    const CAPACITY: usize = 2 * 1024 * 1024;

    pub fn new() -> Self {
        Self {
            memory: Vec::with_capacity(Self::CAPACITY),
        }
    }

    pub fn parse(&mut self, buffer: Vec<u8>) -> Option<RgbPacket> {
        if self.memory.len() + buffer.len() > Self::CAPACITY {
            self.memory.clear();
            return None;
        }

        self.memory.extend(buffer);

        if self.memory.len() <= (size_of::<RawRgbPacketHeader>() + size_of::<RawRgbPacketFooter>())
        {
            return None;
        }

        let footer = unsafe {
            read_unaligned(
                &self.memory[self.memory.len() - size_of::<RawRgbPacketFooter>()..] as *const [u8]
                    as *const RawRgbPacketFooter,
            )
        };

        if footer.magic_header != 0x39393939 || footer.magic_footer != 0x42424242 {
            return None;
        }

        let header = unsafe {
            read_unaligned(self.memory.as_slice() as *const [u8] as *const RawRgbPacketHeader)
        };

        if self.memory.len() != footer.packet_size as usize
            || header.sequence != footer.sequence
            || (self.memory.len()
                - size_of::<RawRgbPacketHeader>()
                - size_of::<RawRgbPacketFooter>())
                < footer.filler_length as usize
        {
            self.memory.clear();
            return None;
        }

        let mut jpeg_length = 0;
        let length_no_filler = self.memory.len()
            - size_of::<RawRgbPacketHeader>()
            - size_of::<RawRgbPacketFooter>()
            - footer.filler_length as usize;
        let jpeg_buffer = &self.memory[size_of::<RawRgbPacketHeader>()..];

        for index in 0..4 {
            if length_no_filler < index + 2 {
                break;
            }

            let eoi = length_no_filler - index;

            if jpeg_buffer[eoi - 2] == 0xff && jpeg_buffer[eoi - 1] == 0xd9 {
                jpeg_length = eoi;
            }
        }

        if jpeg_length == 0 {
            self.memory.clear();
            return None;
        }

        let packet = RgbPacket {
            sequence: header.sequence,
            timestamp: footer.timestamp,
            exposure: footer.exposure,
            gain: footer.gain,
            gamma: footer.gamma,
            jpeg_buffer: jpeg_buffer[..jpeg_length].to_vec(),
        };

        self.memory.clear();

        Some(packet)
    }
}
