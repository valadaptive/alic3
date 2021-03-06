use std::cell::RefCell;
use std::io::prelude::*;
use std::io::{Stdin, Stdout};

use byteorder::ReadBytesExt;
use byteorder::{BigEndian, WriteBytesExt};

use crate::bit_twiddling::*;
use crate::opcode::*;

const MEMORY_SIZE: usize = (u16::MAX as usize) + 1;

/// Memory addresses of all memory-mapped registers
struct MemRegisters {}

impl MemRegisters {
    /// Program status register.
    /// `PSR[15]` is 1 if running in user mode, 0 if in supervisor mode.
    /// `PSR[10:8]` specifies the priority level of the currently running process.
    /// `PSR[2:0]` holds condition codes (set depending on whether the previous result was positive, negative, or zero)
    const PSR: u16 = 0xFFFC;
    /// Machine control register.
    /// When the top bit is cleared, the emulator exits.
    const MCR: u16 = 0xFFFE;

    /// Keyboard status register.
    /// The top bit is set when the keyboard has more data to read. Reading that data will clear the top bit.
    const KBSR: u16 = 0xFE00;
    /// Keyboard data register.
    /// Holds the key code of the key that was most recently pressed.
    const KBDR: u16 = 0xFE02;

    /// Display status register.
    /// The top bit is set when the display is ready for data to be written to it. Currently that's always the case.
    const DSR: u16 = 0xFE04;
    /// Display data register.
    /// Writing a character into this register will print that character to the display.
    const DDR: u16 = 0xFE06;
}

struct Memory<Input: Read, Output: Write> {
    memory: [u16; MEMORY_SIZE],
    keyboard_io: RefCell<KeyboardIO<Input>>,
    stdout: Output,
}

impl<Input: Read, Output: Write> Memory<Input, Output> {
    fn get(&self, addr: u16) -> u16 {
        match addr {
            MemRegisters::KBSR => self.keyboard_io.borrow_mut().read_kbsr(),
            MemRegisters::KBDR => self.keyboard_io.borrow_mut().read_kbdr(),
            // The display is always ready for more data
            MemRegisters::DSR => 0x8000,
            _ => self.memory[addr as usize],
        }
    }

    fn set(&mut self, addr: u16, value: u16) {
        match addr {
            // Ignore writes into status/read-only registers
            MemRegisters::KBSR | MemRegisters::KBDR | MemRegisters::DSR => (),
            MemRegisters::DDR => {
                // Because we're in raw mode so we can get each character individually, line feeds don't reset the
                // cursor position to the left. Do that manually.
                // TODO: proper terminal driver
                if value == (b'\n' as u16) {
                    let _ = self.stdout.write_u8(b'\r');
                }
                // Ignore potential errors writing to stdout
                let _ = self.stdout.write_u8((value & 0xFF) as u8);
                // TODO: flushing stdout after every character seems expensive but also the only way to ensure proper
                // emulation.
                let _ = self.stdout.flush();
            }
            _ => {
                self.memory[addr as usize] = value;
            }
        };
    }
}

struct KeyboardIO<T: Read> {
    need_more_input: bool,
    kbsr: bool,
    kbdr: u16,
    stdin: T,
}

impl<T: Read> KeyboardIO<T> {
    fn new(stdin: T) -> Self {
        KeyboardIO {
            need_more_input: true,
            kbsr: false,
            kbdr: 0,
            stdin,
        }
    }

    fn update_input(&mut self) {
        if let Ok(keycode) = self.stdin.read_u8() {
            self.kbdr = keycode as u16;
            self.kbsr = true;
            self.need_more_input = false;
        } else {
            self.kbsr = false;
        }
    }

    fn read_kbsr(&mut self) -> u16 {
        if self.need_more_input {
            self.update_input();
        }

        if self.kbsr {
            0x8000
        } else {
            0x0000
        }
    }

    fn read_kbdr(&mut self) -> u16 {
        if self.need_more_input {
            self.update_input();
        }

        self.need_more_input = true;
        self.kbdr
    }
}

const COND_NEGATIVE: u16 = 0b100;
const COND_ZERO: u16 = 0b010;
const COND_POSITIVE: u16 = 0b001;

pub struct Cpu<Input: Read, Output: Write> {
    /// All CPU registers
    registers: [u16; 8],
    /// RAM
    memory: Memory<Input, Output>,
    /// Program counter
    pub pc: u16,
    /// The saved user mode stack pointer
    saved_usp: u16,
    /// The saved supervisor mode stack pointer
    saved_ssp: u16,
}

impl<Input: Read, Output: Write> Cpu<Input, Output> {
    pub fn new(stdin: Input, stdout: Output) -> Self {
        let mut mem_raw = [0u16; MEMORY_SIZE];
        // Initialize MCR so we don't halt immediately
        mem_raw[MemRegisters::MCR as usize] = 0xFFFF;
        Cpu {
            registers: [0u16; 8],
            memory: Memory {
                memory: mem_raw,
                keyboard_io: RefCell::new(KeyboardIO::new(stdin)),
                stdout,
            },
            pc: 0u16,
            // We decrement the stack pointer before writing and increment after reading, so these start at 1 after the
            // addresses at which the stack actually begins. Stacks grow downwards in memory.
            saved_ssp: 0x3000,
            saved_usp: 0xFE00,
        }
    }

    fn address_accessible(&self, addr: u16) -> bool {
        // Only protect memory if not in privileged mode
        if get_bits::<15, 15>(self.memory.get(MemRegisters::PSR)) == 0 {
            return true;
        }

        addr >= 0x3000 && addr <= 0xFDFF
    }

    fn enter_supervisor_mode(&mut self) {
        let old_psr = self.memory.get(MemRegisters::PSR);
        if get_bits::<15, 15>(self.memory.get(MemRegisters::PSR)) == 1 {
            // Switch from the user stack pointer to the system stack pointer
            self.saved_usp = self.registers[6];
            self.registers[6] = self.saved_ssp;
            // Clear the "is user mode" bit of the PSR
            self.memory.set(
                MemRegisters::PSR,
                self.memory.get(MemRegisters::PSR) & !(1 << 15),
            );
        }

        // Push old MemRegisters::PSR and PC to stack
        self.registers[6] -= 1;
        self.memory.set(self.registers[6], old_psr);
        self.registers[6] -= 1;
        self.memory.set(self.registers[6], self.pc);
    }

    fn handle_exception(&mut self, exception_vector: u8) {
        // TODO: it's possible to trigger an exception in an exception handler, etc. ad infinitum.
        // A triple-fault handler would technically be against spec but probably useful.
        self.enter_supervisor_mode();
        let exception_addr = (exception_vector as u16) | 0x0100;
        self.pc = exception_addr;
    }

    fn handle_interrupt(&mut self, interrupt_vector: u8, priority_level: u16) {
        if priority_level <= self.get_priority_level() {
            return;
        }

        self.handle_exception(interrupt_vector);
        self.set_priority_level(priority_level);
    }

    fn get_priority_level(&self) -> u16 {
        return (self.memory.get(MemRegisters::PSR) >> 8) & 0b111;
    }

    fn set_priority_level(&mut self, level: u16) {
        assert!(level <= 7);
        self.memory.set(
            MemRegisters::PSR,
            (self.memory.get(MemRegisters::PSR) & !0b11100000000) | (level << 8),
        );
    }

    /// Read the contents of the register specified in the high bits of an instruction (a common operation).
    /// This is usually the destination register for an operation, but can be the source register if we're starved for
    /// bits.
    fn get_reg_hi(&self, instruction: u16) -> u16 {
        self.registers[get_bits::<9, 11>(instruction) as usize]
    }

    /// Read the contents of the register specified in the low bits of an instruction (a common operation).
    /// This is the source/base register for most operations.
    fn get_reg_lo(&self, instruction: u16) -> u16 {
        self.registers[get_bits::<6, 8>(instruction) as usize]
    }

    /// Decode and execute a single instruction. Specialized based on opcode.
    pub fn execute_instruction<const OP: u8>(&mut self, instruction: u16) {
        let opcode = Opcode::from_int(OP);

        match opcode {
            // These opcodes all set the condition codes after doing some work
            Opcode::Add
            | Opcode::And
            | Opcode::Ld
            | Opcode::Ldi
            | Opcode::Ldr
            | Opcode::Lea
            | Opcode::Not => {
                let result = match opcode {
                    Opcode::Add | Opcode::And => {
                        let src_value_2 = if get_bits::<5, 5>(instruction) == 1 {
                            // immediate mode
                            sign_extend::<5>(get_bits::<0, 4>(instruction) as i16) as u16
                        } else {
                            self.registers[get_bits::<0, 2>(instruction) as usize]
                        };

                        match opcode {
                            Opcode::Add => {
                                u16::wrapping_add(self.get_reg_lo(instruction), src_value_2)
                            }
                            Opcode::And => self.get_reg_lo(instruction) & src_value_2,
                            _ => unreachable!(),
                        }
                    }

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
                                self.memory.get(addr)
                            }
                            // load indirect
                            Opcode::Ldi => {
                                if !self.address_accessible(addr) {
                                    self.handle_exception(0x02);
                                    return;
                                }
                                let indirect_addr = self.memory.get(addr);
                                if !self.address_accessible(addr) {
                                    self.handle_exception(0x02);
                                    return;
                                }
                                self.memory.get(indirect_addr)
                            }
                            _ => unreachable!(),
                        }
                    }

                    Opcode::Ldr => {
                        let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                        let addr = u16::wrapping_add(self.get_reg_lo(instruction), offset as u16);
                        if !self.address_accessible(addr) {
                            self.handle_exception(0x02);
                            return;
                        }
                        self.memory.get(addr)
                    }

                    Opcode::Not => !self.get_reg_lo(instruction),

                    _ => unreachable!(),
                };

                self.registers[get_bits::<9, 11>(instruction) as usize] = result;

                // Third edition: LEA doesn't set condition codes
                #[cfg(not(feature = "third_edition"))]
                if let Opcode::Lea = opcode {
                    return;
                }

                // Replace lower 3 bits of PSR with the new condition bits
                self.memory.set(
                    MemRegisters::PSR,
                    (self.memory.get(MemRegisters::PSR) & !0b111)
                        | match (result as i16).signum() {
                            -1 => COND_NEGATIVE,
                            0 => COND_ZERO,
                            1 => COND_POSITIVE,
                            _ => unreachable!(),
                        },
                );
            }

            Opcode::Br => {
                let nzp = get_bits::<9, 11>(instruction);
                // This will only match the lowest 3 bits, which store the condition codes
                if (nzp & self.memory.get(MemRegisters::PSR)) > 0 {
                    let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                    self.pc = u16::wrapping_add(self.pc, pc_offset as u16);
                }
            }

            Opcode::Jmp => {
                // TODO: it looks like there's no protection against jumping into the middle of privileged code.
                // The book has nothing to say on the matter but it may be useful to implement protections against that.
                self.pc = self.get_reg_lo(instruction);

                // Undocumented JMPT/RTT instruction. No idea who came up with this or where it is or isn't implemented.
                // If the LSB of the instruction is set, the user mode bit is set to 1.
                // The only other way to do this is to modify the PSR directly.
                // Commenting this out for now because I have no idea about the specifics of its functionality.
                /* if get_bits::<0, 0>(instruction) == 1 {
                    if get_bits::<15, 15>(self.memory.get(MemRegisters::PSR)) == 1 {
                        // Tried to execute JMPT/RTT from user mode--trigger a privilege mode violation
                        // TODO: do the emulators actually do this? do they even check the privilege bit at all?
                        // how has no one actually fully implemented this architecture?
                        self.handle_exception(0x00);
                        return;
                    }

                    // Set user-mode MemRegisters::PSR bit
                    self.memory.get(MemRegisters::PSR) |= 1 << 15;
                } */
            }

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
            }

            Opcode::St | Opcode::Sti => {
                let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
                let mut addr = u16::wrapping_add(self.pc, pc_offset as u16);

                if !self.address_accessible(addr) {
                    self.handle_exception(0x02);
                    return;
                }

                // read address from memory if indirect store
                if let Opcode::Sti = opcode {
                    addr = self.memory.get(addr);
                    if !self.address_accessible(addr) {
                        self.handle_exception(0x02);
                    }
                }

                self.memory.set(addr, self.get_reg_hi(instruction));
            }

            Opcode::Str => {
                let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
                let addr = u16::wrapping_add(self.get_reg_lo(instruction), offset as u16);
                if !self.address_accessible(addr) {
                    self.handle_exception(0x02);
                    return;
                }
                self.memory.set(addr, self.get_reg_hi(instruction));
            }

            Opcode::Rti => {
                if get_bits::<15, 15>(self.memory.get(MemRegisters::PSR)) == 1 {
                    // Tried to execute RTI from user mode--trigger a privilege mode violation
                    self.handle_exception(0x00);
                    return;
                }

                // Restore PC from supervisor stack pointer
                self.pc = self.memory.get(self.registers[6]);
                self.registers[6] += 1;

                // TODO: the book says to pop the system stack before restoring the PSR. Why?
                let new_psr = self.memory.get(self.registers[6]);
                self.registers[6] += 1;
                self.memory.set(MemRegisters::PSR, new_psr);

                if get_bits::<15, 15>(self.memory.get(MemRegisters::PSR)) == 1 {
                    // We are now back in user mode
                    self.saved_ssp = self.registers[6];
                    self.registers[6] = self.saved_usp;
                }
            }

            Opcode::Reserved => {
                self.handle_exception(0x01);
            }

            Opcode::Trap => {
                // In the third-edition LC-3, the old PC and PSR are stored on the stack and supervisor mode is entered
                #[cfg(feature = "third_edition")]
                self.enter_supervisor_mode();

                // In the second-edition LC-3, TRAP sets R7 to the previous PC
                #[cfg(not(feature = "third_edition"))]
                {
                    self.registers[7] = self.pc;
                }

                // Jump into code specified by trap vector table
                self.pc = self.memory.get(get_bits::<0, 7>(instruction));
            }
        }
    }

    pub fn step(&mut self) {
        let instruction = self.memory.get(self.pc);
        // println!("PC: {:#06x}, instruction: {:#06x} ({})", self.pc, instruction, disassemble_instruction(instruction));
        self.pc = self.pc.wrapping_add(1);
        let op = get_bits::<12, 15>(instruction);

        // We take advantage of const generics to compile 16 specialized code paths through execute_instruction
        // depending on which instruction it is. This seemingly redundant code is necessary because these are basically
        // 16 different functions--the opcode's essentially a template parameter which must be known at compile time.
        match op {
            0 => self.execute_instruction::<0>(instruction),
            1 => self.execute_instruction::<1>(instruction),
            2 => self.execute_instruction::<2>(instruction),
            3 => self.execute_instruction::<3>(instruction),
            4 => self.execute_instruction::<4>(instruction),
            5 => self.execute_instruction::<5>(instruction),
            6 => self.execute_instruction::<6>(instruction),
            7 => self.execute_instruction::<7>(instruction),
            8 => self.execute_instruction::<8>(instruction),
            9 => self.execute_instruction::<9>(instruction),
            10 => self.execute_instruction::<10>(instruction),
            11 => self.execute_instruction::<11>(instruction),
            12 => self.execute_instruction::<12>(instruction),
            13 => self.execute_instruction::<13>(instruction),
            14 => self.execute_instruction::<14>(instruction),
            15 => self.execute_instruction::<15>(instruction),
            _ => unreachable!(),
        };
    }

    pub fn should_halt(&mut self) -> bool {
        self.memory.get(MemRegisters::MCR) & (1 << 15) == 0
    }

    pub fn load_program<F>(&mut self, mut file: F) -> std::io::Result<()>
    where
        F: Read,
    {
        let mut buf = Vec::new();
        let origin = file.read_u16::<BigEndian>()? as usize;
        file.read_to_end(&mut buf)?;
        buf.chunks(2).enumerate().for_each(|(i, chunk)| {
            self.memory.memory[i + origin] = ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        });
        Ok(())
    }
}
