use std::u32;

use crate::{config::DEPTH_FRAME_SIZE, packet::DepthPacket, ReadUnaligned};

/** Footer of a depth packet. */
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
    processed_packets: i64,
    current_sequence: u32,
    current_subsequence: u32,
}

impl DepthStreamParser {
    const MEMORY_CAPACITY: usize = DEPTH_FRAME_SIZE * 11 / 8;
    const WORKER_CAPACITY: usize = Self::MEMORY_CAPACITY * 10;

    pub fn new() -> Self {
        Self {
            memory: Vec::with_capacity(Self::MEMORY_CAPACITY),
            worker: Vec::with_capacity(Self::WORKER_CAPACITY),
            processed_packets: -1,
            current_sequence: 0,
            current_subsequence: 0,
        }
    }

    pub fn parse(&mut self, mut buffer: Vec<u8>) -> Option<DepthPacket> {
        if buffer.is_empty() {
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

        let mut result = None;

        if let Some(footer) = footer {
            if footer.length as usize == self.worker.len() {
                if self.current_sequence != footer.sequence {
                    if self.current_subsequence == 0x3ff {
                        result = Some(DepthPacket {
                            sequence: self.current_sequence,
                            timestamp: footer.timestamp,
                            buffer: self.memory.drain(..).collect(),
                        });

                        self.processed_packets += 1;

                        if self.processed_packets == 0 {
                            self.processed_packets = self.current_sequence as i64;
                        }

                        let diff = self.current_sequence - self.processed_packets as u32;
                        const INTERVAL: u32 = 30;

                        if (self.current_sequence % INTERVAL == 0 && diff != 0) || diff >= INTERVAL
                        {
                            self.processed_packets = self.current_sequence as i64;
                        }
                    }

                    self.current_sequence = footer.sequence;
                    self.current_subsequence = 0;
                }

                self.current_subsequence |= 1 << footer.subsequence;

                if (footer.subsequence * footer.length) as usize <= Self::MEMORY_CAPACITY {
                    todo!();
                    //self.memory.extend();
                }
            }

            self.worker.clear();
        }

        result
    }
}
