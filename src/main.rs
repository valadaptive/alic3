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

struct Registers([u16; 8]);

impl Registers {
    fn get(&self, idx: usize) -> u16 {
        self.0[idx]
    }

    fn set(&mut self, idx: usize, value: u16) {
        self.0[idx] = value;
    }
}

const COND_NEGATIVE: u16 = 0b100;
const COND_ZERO: u16 = 0b010;
const COND_POSITIVE: u16 = 0b001;

pub struct CpuState {
    registers: Registers,
    memory: Memory,
    pc: u16,
    cc: u16,
    psr: u16
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
    // Only protect memory if PSR[15] == 1
    if get_bits::<15, 15>(cpu_state.psr) == 0 {
        return true;
    }

    addr >= 0x3000 && addr <= 0xFDFF
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
            let dst_register = get_bits::<9, 11>(instruction) as usize;

            let result = match opcode {
                Opcode::Add | Opcode::And => {
                    let src_register_1 = get_bits::<6, 8>(instruction) as usize;
                    let src_value_2 = if get_bits::<5, 5>(instruction) == 1 { // immediate mode
                        get_bits::<0, 4>(instruction)
                    } else {
                        let src_register_2 = get_bits::<0, 2>(instruction) as usize;
                        cpu_state.registers.get(src_register_2)
                    };

                    match opcode {
                        Opcode::Add => u16::wrapping_add(cpu_state.registers.get(src_register_1), src_value_2),
                        Opcode::And => cpu_state.registers.get(src_register_1) & src_value_2,
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
                    let base_register = get_bits::<6, 8>(instruction) as usize;
                    let base_value = cpu_state.registers.get(base_register);
                    let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                    let addr = u16::wrapping_add(base_value, offset as u16);
                    if !address_accessible(addr, cpu_state) {
                        unimplemented!()
                    }
                    cpu_state.memory.get(addr)
                },

                Opcode::Not => {
                    let src_register = get_bits::<6, 8>(instruction) as usize;
                    let src_value = cpu_state.registers.get(src_register);

                    !src_value
                },

                _ => unreachable!()
            };

            cpu_state.registers.set(dst_register, result);
            cpu_state.cc = match (result as i16).signum() {
                -1 => COND_NEGATIVE,
                0 => COND_ZERO,
                1 => COND_POSITIVE,
                _ => unreachable!()
            }
        },

        Opcode::Br => {
            let nzp = get_bits::<9, 11>(instruction);
            if (nzp & cpu_state.cc) > 0 {
                let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                cpu_state.pc = u16::wrapping_add(cpu_state.pc, pc_offset as u16);
            }
        },

        Opcode::Jmp => {
            let base_register = get_bits::<6, 8>(instruction) as usize;
            let base_value = cpu_state.registers.get(base_register);
            cpu_state.pc = base_value;
        },

        Opcode::Jsr => {
            cpu_state.registers.set(7, cpu_state.pc);
            if get_bits::<11, 11>(instruction) == 1 {
                // JSR: PC-relative
                let pc_offset = sign_extend::<11>(get_bits::<0, 10>(instruction) as i16);
                cpu_state.pc = u16::wrapping_add(cpu_state.pc, pc_offset as u16);
            } else {
                // JSRR: absolute
                let base_register = get_bits::<6, 8>(instruction) as usize;
                let base_value = cpu_state.registers.get(base_register);
                cpu_state.pc = base_value;
            }
        },

        Opcode::St | Opcode::Sti => {
            let src_register = get_bits::<9, 11>(instruction) as usize;
            let src_value = cpu_state.registers.get(src_register);
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

            cpu_state.memory.set(addr, src_value);
        },

        Opcode::Str => {
            let base_register = get_bits::<6, 8>(instruction) as usize;
            let base_value = cpu_state.registers.get(base_register);
            let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
            let addr = u16::wrapping_add(base_value, offset as u16);
            if !address_accessible(addr, cpu_state) {
                unimplemented!()
            }
            let src_register = get_bits::<9, 11>(instruction) as usize;
            let src_value = cpu_state.registers.get(src_register);
            cpu_state.memory.set(addr, src_value);
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
