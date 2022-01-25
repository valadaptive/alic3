use std::fs::File;
use std::io::prelude::*;
use std::env::args;

mod bit_twiddling;
mod opcode;

use byteorder::LittleEndian;
use byteorder::ReadBytesExt;

use crate::bit_twiddling::*;
use crate::opcode::*;

const MEMORY_SIZE: usize = u16::MAX as usize;

struct Memory {
    memory: [u16; MEMORY_SIZE]
}

impl Memory {
    fn get(&self, idx: u16) -> u16 {
        self.memory[idx as usize]
    }

    fn set(&mut self, idx: u16, value: u16) {
        self.memory[idx as usize] = value;
    }
}

const COND_NEGATIVE: u16 = 0b100;
const COND_ZERO: u16 = 0b010;
const COND_POSITIVE: u16 = 0b001;

pub struct Cpu {
    /// All CPU registers
    registers: [u16; 8],
    /// RAM
    memory: Memory,
    /// Program counter
    pc: u16,
    /// Program status register.
    /// `psr[15]` is 1 if running in user mode, 0 if in supervisor mode.
    /// `psr[10:8]` specifies the priority level of the currently running process.
    /// `psr[2:0]` holds condition codes (set depending on whether the previous result was positive, negative, or zero)
    psr: u16,
    /// The saved user mode stack pointer
    saved_usp: u16,
    /// The saved supervisor mode stack pointer
    saved_ssp: u16
}

impl Cpu {
    fn new() -> Self {
        Cpu {
            registers: [0u16; 8],
            memory: Memory {memory: [0u16; MEMORY_SIZE]},
            pc: 0u16,
            psr: 0u16,
            saved_ssp: 0u16,
            saved_usp: 0u16
        }
    }

    fn address_accessible(&self, addr: u16) -> bool {
        // Only protect memory if not in privileged mode
        if get_bits::<15, 15>(self.psr) == 0 {
            return true;
        }

        addr >= 0x3000 && addr <= 0xFDFF
    }

    fn get_reg_hi(&self, instruction: u16) -> u16 {
        self.registers[get_bits::<9, 11>(instruction) as usize]
    }

    fn get_reg_lo(&self, instruction: u16) -> u16 {
        self.registers[get_bits::<6, 8>(instruction) as usize]
    }

    pub fn execute_instruction<const OP: u8>(&mut self, instruction: u16) {
        let opcode = Opcode::from_int(OP);

        match opcode {
            Opcode::Add |
            Opcode::And |
            Opcode::Ld |
            Opcode::Ldi |
            Opcode::Ldr |
            Opcode::Lea |
            Opcode::Not => {
                let result = match opcode {
                    Opcode::Add | Opcode::And => {
                        let src_value_2 = if get_bits::<5, 5>(instruction) == 1 { // immediate mode
                            get_bits::<0, 4>(instruction)
                        } else {
                            self.registers[get_bits::<0, 2>(instruction) as usize]
                        };

                        match opcode {
                            Opcode::Add => u16::wrapping_add(self.get_reg_lo(instruction), src_value_2),
                            Opcode::And => self.get_reg_lo(instruction) & src_value_2,
                            _ => unreachable!(),
                        }
                    },

                    Opcode::Ld | Opcode::Ldi | Opcode::Lea => {
                        let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                        let addr = u16::wrapping_add(self.pc, pc_offset as u16);

                        match opcode {
                            // load effective address: just return the address
                            Opcode::Lea => addr,
                            // load direct
                            Opcode::Ld => {
                                if !self.address_accessible(addr) {
                                    unimplemented!()
                                }
                                self.memory.get(addr)
                            },
                            // load indirect
                            Opcode::Ldi => {
                                if !self.address_accessible(addr) {
                                    unimplemented!()
                                }
                                let indirect_addr = self.memory.get(addr);
                                if !self.address_accessible(indirect_addr) {
                                    unimplemented!()
                                }
                                self.memory.get(indirect_addr)
                            },
                            _ => unreachable!()
                        }
                    },

                    Opcode::Ldr => {
                        let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                        let addr = u16::wrapping_add( self.get_reg_lo(instruction), offset as u16);
                        if !self.address_accessible(addr) {
                            unimplemented!()
                        }
                        self.memory.get(addr)
                    },

                    Opcode::Not => {
                        !self.get_reg_lo(instruction)
                    },

                    _ => unreachable!()
                };

                self.registers[get_bits::<9, 11>(instruction) as usize] = result;
                // Replace lower 3 bits of PSR with the new condition bits
                self.psr = (self.psr & !0b111) | match (result as i16).signum() {
                    -1 => COND_NEGATIVE,
                    0 => COND_ZERO,
                    1 => COND_POSITIVE,
                    _ => unreachable!()
                }
            },

            Opcode::Br => {
                let nzp = get_bits::<9, 11>(instruction);
                // This will only match the lowest 3 bits, which store the condition codes
                if (nzp & self.psr) > 0 {
                    let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                    self.pc = u16::wrapping_add(self.pc, pc_offset as u16);
                }
            },

            Opcode::Jmp => {
                self.pc = self.get_reg_lo(instruction);
            },

            Opcode::Jsr => {
                let old_pc = self.pc;

                if get_bits::<11, 11>(instruction) == 1 {
                    // JSR: PC-relative
                    let pc_offset = sign_extend::<11>(get_bits::<0, 10>(instruction) as i16);
                    self.pc = u16::wrapping_add(self.pc, pc_offset as u16);
                } else {
                    // JSRR: absolute
                    self.pc = self.get_reg_lo(instruction);
                }

                // Make sure to set R7 *after* potentially reading the program counter from it (e.g. a JSRR)
                self.registers[7] = old_pc;
            },

            Opcode::St | Opcode::Sti => {
                let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                let mut addr = u16::wrapping_add(self.pc, pc_offset as u16);

                if !self.address_accessible(addr) {
                    unimplemented!()
                }

                // read address from memory if indirect store
                if let Opcode::Sti = opcode {
                    addr = self.memory.get(addr);
                    if !self.address_accessible(addr) {
                        unimplemented!()
                    }
                }

                self.memory.set(addr, self.get_reg_hi(instruction));
            },

            Opcode::Str => {
                let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                let addr = u16::wrapping_add(self.get_reg_lo(instruction), offset as u16);
                if !self.address_accessible(addr) {
                    unimplemented!()
                }
                self.memory.set(addr, self.get_reg_hi(instruction));
            },

            Opcode::Rti => unimplemented!(),

            Opcode::Reserved => {
                panic!("illegal opcode")
            },

            Opcode::Trap => {
                let old_psr = self.psr;
                if get_bits::<15, 15>(self.psr) == 1 {
                    // Switch from the user stack pointer to the system stack pointer
                    self.saved_usp = self.registers[6];
                    self.registers[6] = self.saved_ssp;
                    // Zero out the 15th PSR bit (the "in user mode" bit)
                    self.psr &= !(1 << 15);
                }
                // Push old PSR and PC to stack
                // TODO: do we increment before or after?
                self.memory.set(self.registers[6], old_psr);
                self.registers[6] += 1;
                self.memory.set(self.registers[6], self.pc);
                self.registers[6] += 1;

                // Jump into code specified by trap vector table
                // TODO: does this mean we don't increment the instruction pointer?
                self.pc = self.memory.get(get_bits::<0, 7>(instruction));
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 2 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let mut file = File::open(&args[1])?;
    let mut cpu_state = Cpu::new();
    let mut buf: [u8; MEMORY_SIZE * 2] = [0u8; MEMORY_SIZE * 2];
    let origin = file.read_u16::<LittleEndian>()? as usize;
    file.read(&mut buf)?;
    buf.chunks(2)
        .enumerate()
        .for_each(|(i, chunk)| {
            cpu_state.memory.memory[i + origin] = ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        });
    Ok(())
}
