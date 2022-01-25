use std::fs::File;
use std::io::prelude::*;

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

pub struct CpuState {
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

#[repr(u8)]
#[derive(PartialEq, Eq)]
enum Opcode {
    Br = 0,
    Add = 1,
    Ld = 2,
    St = 3,
    Jsr = 4,
    And = 5,
    Ldr = 6,
    Str = 7,
    Rti = 8,
    Not = 9,
    Ldi = 10,
    Sti = 11,
    Jmp = 12, // also RET
    Reserved = 13,
    Lea = 14,
    Trap = 15,
}

impl Opcode {
    const fn from_int(i: u8) -> Self {
        match i {
            0 => Opcode::Br,
            1 => Opcode::Add,
            2 => Opcode::Ld,
            3 => Opcode::St,
            4 => Opcode::Jsr,
            5 => Opcode::And,
            6 => Opcode::Ldr,
            7 => Opcode::Str,
            8 => Opcode::Rti,
            9 => Opcode::Not,
            10 => Opcode::Ldi,
            11 => Opcode::Sti,
            12 => Opcode::Jmp, // also RET
            13 => Opcode::Reserved,
            14 => Opcode::Lea,
            15 => Opcode::Trap,
            _ => panic!("opcode outside range")
        }
    }
}

fn get_bits<const START: usize, const END: usize>(n: u16) -> u16 {
    assert!(END <= 15 && START <= END, "start and end bits out of bounds");
    let mask = u16::MAX >> (15 - (END - START));
    (n >> START) & mask
}

fn sign_extend<const NUM_BITS: usize>(n: i16) -> i16 {
    assert!(NUM_BITS <= 16);
    (n << (16 - NUM_BITS)) >> (16 - NUM_BITS)
}

fn address_accessible(addr: u16, cpu_state: &CpuState) -> bool {
    // Only protect memory if not in privileged mode
    if get_bits::<15, 15>(cpu_state.psr) == 0 {
        return true;
    }

    addr >= 0x3000 && addr <= 0xFDFF
}

fn get_reg_hi(instruction: u16, cpu_state: &CpuState) -> u16 {
    cpu_state.registers[get_bits::<9, 11>(instruction) as usize]
}

fn get_reg_lo(instruction: u16, cpu_state: &CpuState) -> u16 {
    cpu_state.registers[get_bits::<6, 8>(instruction) as usize]
}

pub fn execute_instruction<const OP: u8>(instruction: u16, cpu_state: &mut CpuState) {
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
                        cpu_state.registers[get_bits::<0, 2>(instruction) as usize]
                    };

                    match opcode {
                        Opcode::Add => u16::wrapping_add(get_reg_lo(instruction, cpu_state), src_value_2),
                        Opcode::And => get_reg_lo(instruction, cpu_state) & src_value_2,
                        _ => unreachable!(),
                    }
                },

                Opcode::Ld | Opcode::Ldi | Opcode::Lea => {
                    let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                    let addr = u16::wrapping_add(cpu_state.pc, pc_offset as u16);

                    match opcode {
                        // load effective address: just return the address
                        Opcode::Lea => addr,
                        // load direct
                        Opcode::Ld => {
                            if !address_accessible(addr, cpu_state) {
                                unimplemented!()
                            }
                            cpu_state.memory.get(addr)
                        },
                        // load indirect
                        Opcode::Ldi => {
                            if !address_accessible(addr, cpu_state) {
                                unimplemented!()
                            }
                            let indirect_addr = cpu_state.memory.get(addr);
                            if !address_accessible(indirect_addr, cpu_state) {
                                unimplemented!()
                            }
                            cpu_state.memory.get(indirect_addr)
                        },
                        _ => unreachable!()
                    }
                },

                Opcode::Ldr => {
                    let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                    let addr = u16::wrapping_add( get_reg_lo(instruction, cpu_state), offset as u16);
                    if !address_accessible(addr, cpu_state) {
                        unimplemented!()
                    }
                    cpu_state.memory.get(addr)
                },

                Opcode::Not => {
                    !get_reg_lo(instruction, cpu_state)
                },

                _ => unreachable!()
            };

            cpu_state.registers[get_bits::<9, 11>(instruction) as usize] = result;
            // Replace lower 3 bits of PSR with the new condition bits
            cpu_state.psr = (cpu_state.psr & !0b111) | match (result as i16).signum() {
                -1 => COND_NEGATIVE,
                0 => COND_ZERO,
                1 => COND_POSITIVE,
                _ => unreachable!()
            }
        },

        Opcode::Br => {
            let nzp = get_bits::<9, 11>(instruction);
            // This will only match the lowest 3 bits, which store the condition codes
            if (nzp & cpu_state.psr) > 0 {
                let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                cpu_state.pc = u16::wrapping_add(cpu_state.pc, pc_offset as u16);
            }
        },

        Opcode::Jmp => {
            cpu_state.pc = get_reg_lo(instruction, cpu_state);
        },

        Opcode::Jsr => {
            let old_pc = cpu_state.pc;

            if get_bits::<11, 11>(instruction) == 1 {
                // JSR: PC-relative
                let pc_offset = sign_extend::<11>(get_bits::<0, 10>(instruction) as i16);
                cpu_state.pc = u16::wrapping_add(cpu_state.pc, pc_offset as u16);
            } else {
                // JSRR: absolute
                cpu_state.pc = get_reg_lo(instruction, cpu_state);
            }

            // Make sure to set R7 *after* potentially reading the program counter from it (e.g. a JSRR)
            cpu_state.registers[7] = old_pc;
        },

        Opcode::St | Opcode::Sti => {
            let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
            let mut addr = u16::wrapping_add(cpu_state.pc, pc_offset as u16);

            if !address_accessible(addr, cpu_state) {
                unimplemented!()
            }

            // read address from memory if indirect store
            if let Opcode::Sti = opcode {
                addr = cpu_state.memory.get(addr);
                if !address_accessible(addr, cpu_state) {
                    unimplemented!()
                }
            }

            cpu_state.memory.set(addr, get_reg_hi(instruction, cpu_state));
        },

        Opcode::Str => {
            let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
            let addr = u16::wrapping_add(get_reg_lo(instruction, cpu_state), offset as u16);
            if !address_accessible(addr, cpu_state) {
                unimplemented!()
            }
            cpu_state.memory.set(addr, get_reg_hi(instruction, cpu_state));
        },

        Opcode::Rti => unimplemented!(),

        Opcode::Reserved => {
            panic!("illegal opcode")
        },

        Opcode::Trap => unimplemented!()
    }
}

fn main() -> std::io::Result<()> {
    println!("Hello, world!");
    Ok(())
}
