// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Lecture du SPD des barrettes (DDR4 EE1004, DDR5 SPD5118) : fabricant JEDEC
//! et données constructeur, pour identifier la RAM avant de lui parler.

use super::smbus::Bus;
use std::thread::sleep;
use std::time::Duration;

pub const JEDEC_KINGSTON: u16 = 0x0117;
pub const JEDEC_KINGSTON_2: u16 = 0x300F;
pub const JEDEC_KINGSTON_3: u16 = 0x3011;

const IO: Duration = Duration::from_millis(1);

#[derive(Clone, Debug)]
pub struct Slot {
    /// 0..7 (adresse SPD 0x50 + index)
    pub index: u8,
    pub ddr5: bool,
    pub jedec: u16,
    /// Premiers octets de la zone « données du fabricant »
    pub manuf: Vec<u8>,
}

impl Slot {
    pub fn is_kingston(&self) -> bool {
        matches!(self.jedec, JEDEC_KINGSTON | JEDEC_KINGSTON_2 | JEDEC_KINGSTON_3)
    }
}

struct Ddr4<'a> {
    bus: &'a Bus,
    addr: u8,
    page: u8,
}

impl Ddr4<'_> {
    fn at(&mut self, a: u16) -> u8 {
        let page = (a >> 8) as u8;
        if page != self.page {
            let _ = self.bus.write_byte_data(0x36 + page, 0x00, 0xFF);
            self.page = page;
            sleep(IO);
        }
        let v = self.bus.read_byte_data(self.addr, a as u8).unwrap_or(0xFF);
        sleep(IO);
        v
    }
}

struct Ddr5<'a> {
    bus: &'a Bus,
    addr: u8,
    page: u8,
}

impl Ddr5<'_> {
    fn at(&mut self, a: u16) -> u8 {
        let page = (a >> 7) as u8;
        if page != self.page {
            let _ = self.bus.write_byte_data(self.addr, 0x0B, page);
            self.page = page;
            sleep(IO);
        }
        let v = self.bus.read_byte_data(self.addr, (a as u8 & 0x7F) | 0x80).unwrap_or(0xFF);
        sleep(IO);
        v
    }
}

fn jedec(hi: u8, lo: u8) -> u16 {
    ((hi as u16) << 8).wrapping_add((lo & 0x7F) as u16).wrapping_sub(1)
}

/// Lit le SPD de chaque emplacement occupé.
pub fn read_slots(bus: &Bus) -> Vec<Slot> {
    let mut out = Vec::new();
    for i in 0..8u8 {
        let addr = 0x50 + i;
        // DDR4 : page 0 puis signature 0x23
        let _ = bus.write_byte(0x36, 0x00);
        sleep(IO);
        if bus.read_byte_data(addr, 0x00).ok() == Some(0x23) {
            let mut s = Ddr4 { bus, addr, page: 0 };
            if s.at(0x02) == 0x0C {
                let j = jedec(s.at(0x140), s.at(0x141));
                let manuf = (0..0x10).map(|k| s.at(0x161 + k)).collect();
                out.push(Slot { index: i, ddr5: false, jedec: j, manuf });
            }
            let _ = bus.write_byte_data(0x36, 0x00, 0xFF); // retour en page 0
            sleep(IO);
            continue;
        }
        // DDR5 : hub SPD5118
        let magic = bus.read_byte_data(addr, 0x00);
        let sensor = bus.read_byte_data(addr, 0x01);
        if let (Ok(0x51), Ok(t)) = (magic, sensor) {
            if t & 0xEF == 0x08 {
                let mut s = Ddr5 { bus, addr, page: 0xFF };
                if s.at(0x02) == 0x12 {
                    let j = jedec(s.at(0x200), s.at(0x201));
                    let manuf = (0..0x20).map(|k| s.at(0x22B + k)).collect();
                    out.push(Slot { index: i, ddr5: true, jedec: j, manuf });
                }
                let _ = bus.write_byte_data(addr, 0x0B, 0);
                sleep(IO);
            }
        }
    }
    crate::log(&format!("SPD : {:?}", out.iter().map(|s| (s.index, s.ddr5, format!("{:04x}", s.jedec))).collect::<Vec<_>>()));
    out
}
