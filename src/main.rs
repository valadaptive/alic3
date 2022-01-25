use std::fs::File;
use std::io::prelude::*;
use std::env::args;
use std::ops::Index;
use std::ops::IndexMut;

mod bit_twiddling;
mod opcode;

use byteorder::LittleEndian;
use byteorder::ReadBytesExt;

use crate::bit_twiddling::*;
use crate::opcode::*;

const MEMORY_SIZE: usize = u16::MAX as usize;

/// Program status register.
/// `psr[15]` is 1 if running in user mode, 0 if in supervisor mode.
/// `psr[10:8]` specifies the priority level of the currently running process.
/// `psr[2:0]` holds condition codes (set depending on whether the previous result was positive, negative, or zero)
const PSR: u16 = 0xFFFC;

struct Memory {
    memory: [u16; MEMORY_SIZE]
}

impl Index<u16> for Memory {
    type Output = u16;

    fn index(&self, index: u16) -> &Self::Output {
        &self.memory[index as usize]
    }
}

impl IndexMut<u16> for Memory {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.memory[index as usize]
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
            // We decrement the stack pointer before writing and increment after reading, so these start at 1 after the
            // addresses at which the stack actually begins. Stacks grow downwards in memory.
            saved_ssp: 0x3000,
            saved_usp: 0xFE00
        }
    }

    fn address_accessible(&self, addr: u16) -> bool {
        // Only protect memory if not in privileged mode
        if get_bits::<15, 15>(self.memory[PSR]) == 0 {
            return true;
        }

        addr >= 0x3000 && addr <= 0xFDFF
    }

    fn enter_supervisor_mode(&mut self) {
        let old_psr = self.memory[PSR];
        if get_bits::<15, 15>(self.memory[PSR]) == 1 {
            // Switch from the user stack pointer to the system stack pointer
            self.saved_usp = self.registers[6];
            self.registers[6] = self.saved_ssp;
            self.memory[PSR] &= !(1 << 15);
        }
        // Push old PSR and PC to stack
        self.registers[6] -= 1;
        self.memory[self.registers[6]] = old_psr;
        self.registers[6] -= 1;
        self.memory[self.registers[6]] =  self.pc;
    }

    fn handle_exception(&mut self, exception_vector: u8) {
        // TODO: it's possible to trigger an exception in an exception handler, etc. ad infinitum.
        // A triple-fault handler would technically be against spec but probably useful.
        self.enter_supervisor_mode();
        let exception_addr = (exception_vector as u16) | 0x0100;
        self.pc = exception_addr;
    }

    fn handle_interrupt(&mut self, interrupt_vector: u8, priority_level: u16) {
        if priority_level <= self.get_priority_level() {return}

        self.handle_exception(interrupt_vector);
        self.set_priority_level(priority_level);
    }

    fn get_priority_level(&self) -> u16 {
        return (self.memory[PSR] >> 8) & 0b111;
    }

    fn set_priority_level(&mut self, level: u16) {
        assert!(level <= 7);
        self.memory[PSR] = (self.memory[PSR] & !0b11100000000) | (level << 8);
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
                                    self.handle_exception(0x02);
                                    return;
                                }
                                self.memory[addr]
                            },
                            // load indirect
                            Opcode::Ldi => {
                                if !self.address_accessible(addr) {
                                    self.handle_exception(0x02);
                                    return;
                                }
                                let indirect_addr = self.memory[addr];
                                if !self.address_accessible(addr) {
                                    self.handle_exception(0x02);
                                    return;
                                }
                                self.memory[indirect_addr]
                            },
                            _ => unreachable!()
                        }
                    },

                    Opcode::Ldr => {
                        let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                        let addr = u16::wrapping_add( self.get_reg_lo(instruction), offset as u16);
                        if !self.address_accessible(addr) {
                            self.handle_exception(0x02);
                            return;
                        }
                        self.memory[addr]
                    },

                    Opcode::Not => {
                        !self.get_reg_lo(instruction)
                    },

                    _ => unreachable!()
                };

                self.registers[get_bits::<9, 11>(instruction) as usize] = result;
                // Replace lower 3 bits of PSR with the new condition bits
                self.memory[PSR] = (self.memory[PSR] & !0b111) | match (result as i16).signum() {
                    -1 => COND_NEGATIVE,
                    0 => COND_ZERO,
                    1 => COND_POSITIVE,
                    _ => unreachable!()
                }
            },

            Opcode::Br => {
                let nzp = get_bits::<9, 11>(instruction);
                // This will only match the lowest 3 bits, which store the condition codes
                if (nzp & self.memory[PSR]) > 0 {
                    let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                    self.pc = u16::wrapping_add(self.pc, pc_offset as u16);
                }
            },

            Opcode::Jmp => {
                self.pc = self.get_reg_lo(instruction);

                // Undocumented JMPT/RTT instruction. No idea who came up with this or where it is or isn't implemented.
                // If the LSB of the instruction is set, the user mode bit is set to 1.
                // The only other way to do this is to modify the PSR directly.
                // Commenting this out for now because I have no idea about the specifics of its functionality.
                /* if get_bits::<0, 0>(instruction) == 1 {
                    if get_bits::<15, 15>(self.memory[PSR]) == 1 {
                        // Tried to execute JMPT/RTT from user mode--trigger a privilege mode violation
                        // TODO: do the emulators actually do this? do they even check the privilege bit at all?
                        // how has no one actually fully implemented this architecture?
                        self.handle_exception(0x00);
                        return;
                    }

                    // Set user-mode PSR bit
                    self.memory[PSR] |= 1 << 15;
                } */
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
                    self.handle_exception(0x02);
                    return;
                }

                // read address from memory if indirect store
                if let Opcode::Sti = opcode {
                    addr = self.memory[addr];
                    if !self.address_accessible(addr) {
                        self.handle_exception(0x02);
                    }
                }

                self.memory[addr] = self.get_reg_hi(instruction);
            },

            Opcode::Str => {
                let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                let addr = u16::wrapping_add(self.get_reg_lo(instruction), offset as u16);
                if !self.address_accessible(addr) {
                    self.handle_exception(0x02);
                    return;
                }
                self.memory[addr] = self.get_reg_hi(instruction);
            },

            Opcode::Rti => {
                if get_bits::<15, 15>(self.memory[PSR]) == 1 {
                    // Tried to execute RTI from user mode--trigger a privilege mode violation
                    self.handle_exception(0x00);
                    return;
                }

                // Restore PC from supervisor stack pointer
                self.pc = self.memory[self.registers[6]];
                self.registers[6] += 1;

                // TODO: the book says to pop the system stack before restoring the PSR. Why?
                let new_psr = self.memory[self.registers[6]];
                self.registers[6] += 1;
                self.memory[PSR] = new_psr;

                if get_bits::<15, 15>(self.memory[PSR]) == 1 {
                    // We are now back in user mode
                    self.saved_ssp = self.registers[6];
                    self.registers[6] = self.saved_usp;
                }
            },

            Opcode::Reserved => {
                self.handle_exception(0x01);
            },

            Opcode::Trap => {
                self.enter_supervisor_mode();

                // Jump into code specified by trap vector table
                // TODO: does this mean we don't increment the instruction pointer?
                self.pc = self.memory[get_bits::<0, 7>(instruction)];
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
