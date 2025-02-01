use std::u32;

use crate::{packet::DepthPacket, ReadUnaligned, DEPTH_SIZE};

/** Footer of a depth packet. */
#[derive(Debug)]
#[repr(C, packed)]
struct DepthSubPacketFooter {
    magic0: u32,
    magic1: u32,
    timestamp: u32,
    sequence: u32,
    subsequence: u32,
    length: u32,
    fields: [u32; 32],
}

impl ReadUnaligned for DepthSubPacketFooter {}

pub struct DepthStreamParser {
    memory: Vec<u8>,
    worker: Vec<u8>,
    processed_packets: Option<u32>,
    current_sequence: u32,
    current_subsequence: u32,
}

impl DepthStreamParser {
    const WORKER_CAPACITY: usize = DEPTH_SIZE * 11 / 8;
    const MEMORY_CAPACITY: usize = Self::WORKER_CAPACITY * 10;

    pub fn new() -> Self {
        Self {
            memory: vec![0u8; Self::MEMORY_CAPACITY],
            worker: Vec::with_capacity(Self::WORKER_CAPACITY),
            processed_packets: None,
            current_sequence: 0,
            current_subsequence: 0,
        }
    }

    pub fn parse(&mut self, mut buffer: Vec<u8>) -> Option<DepthPacket> {
        if buffer.len() == 0 {
            self.worker.clear();
            return None;
        }

        let footer = if self.worker.len() + buffer.len()
            == Self::WORKER_CAPACITY + DepthSubPacketFooter::size()
        {
            DepthSubPacketFooter::read_unaligned(
                &buffer
                    .drain(buffer.len() - DepthSubPacketFooter::size()..)
                    .collect::<Vec<_>>(),
            )
            .ok()
        } else {
            None
        };

        if self.worker.len() + buffer.len() > Self::WORKER_CAPACITY {
            self.worker.clear();
            return None;
        }

        self.worker.extend(buffer);

        let Some(footer) = footer else {
            return None;
        };
        if footer.length as usize != self.worker.len() {
            self.worker.clear();
            return None;
        }

        let mut result = None;

        if self.current_sequence != footer.sequence {
            if self.current_subsequence == 0x3ff {
                result = Some(DepthPacket {
                    sequence: self.current_sequence,
                    timestamp: footer.timestamp,
                    buffer: self.memory.clone(),
                });

                if let Some(processed_packets) = self.processed_packets.as_mut() {
                    *processed_packets += 1;
                } else {
                    self.processed_packets = Some(self.current_sequence);
                }

                let processed_packets = self.processed_packets.as_mut().unwrap();
                let diff = self.current_sequence - *processed_packets;
                const INTERVAL: u32 = 30;

                if (self.current_sequence % INTERVAL == 0 && diff != 0) || diff >= INTERVAL {
                    *processed_packets = self.current_sequence;
                }
            }

            self.current_sequence = footer.sequence;
            self.current_subsequence = 0;
        }

        self.current_subsequence |= 1 << footer.subsequence;

        if (footer.subsequence * footer.length) as usize <= Self::MEMORY_CAPACITY {
            let memory_start = (footer.subsequence * footer.length) as usize;

            self.memory[memory_start..memory_start + footer.length as usize]
                .copy_from_slice(&self.worker);
        }

        self.worker.clear();

        result
    }
}
