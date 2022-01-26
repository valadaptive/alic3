use anyhow::{anyhow, Result};
use std::iter;

use crate::asm_parser::{Instruction, Location, Operation, Program, PseudoOp, RegOrImm, Trap};
use crate::bit_twiddling::truncate;
use crate::opcode::Opcode;

fn resolve_label(pgm: &Program, label: &str) -> Result<u16> {
    pgm.labels
        .get(label)
        .ok_or_else(|| anyhow!("Could not resolve label {}", label))
        .map(|v| *v)
}

fn resolve_location_absolute(pgm: &Program, loc: &Location) -> Result<u16> {
    match loc {
        &Location::Label(label) => resolve_label(pgm, label),
        &Location::Literal(v) => Ok(v),
    }
}

fn resolve_location_relative(pgm: &Program, loc: &Location, line_loc: u16) -> Result<u16> {
    match loc {
        &Location::Label(label) => {
            Ok(u16::wrapping_sub(resolve_label(pgm, label)?, line_loc).wrapping_sub(1))
        }
        &Location::Literal(v) => Ok(v),
    }
}

pub fn assemble(pgm: Program) -> Result<Vec<u16>> {
    let mut machine_code = Vec::<u16>::new();

    // Object files start with the program's memory origin
    machine_code.push(pgm.origin);

    for line in &pgm.lines {
        match &line.instruction {
            Instruction::PseudoOp(PseudoOp::Fill(loc)) => {
                machine_code.push(resolve_location_absolute(&pgm, &loc)?);
            }
            Instruction::PseudoOp(PseudoOp::Blkw(n)) => {
                machine_code.reserve(*n as usize);
                machine_code.extend(iter::repeat(0).take(*n as usize));
            }
            Instruction::PseudoOp(PseudoOp::Stringz(string)) => {
                machine_code.reserve(string.len());
                machine_code.extend(string.iter().map(|char| *char as u16));
            }
            Instruction::PseudoOp(PseudoOp::End) => break,
            Instruction::PseudoOp(PseudoOp::Orig(_)) => return Err(anyhow!("Unexpected .ORIG")),

            Instruction::Operation(operation) => {
                let instruction = match operation {
                    Operation::Add { sr1, sr2, dr } | Operation::And { sr1, sr2, dr } => {
                        let op = match operation {
                            Operation::Add { .. } => Opcode::Add.to_int(),
                            Operation::And { .. } => Opcode::And.to_int(),
                            _ => unreachable!(),
                        };
                        let inst_hi = (op << 12) | (dr.0 << 9) | (sr1.0 << 6);

                        let instruction = match &sr2 {
                            RegOrImm::Register(reg) => inst_hi | reg.0,
                            RegOrImm::Immediate(imm) => inst_hi | (1 << 5) | truncate::<5>(*imm)?,
                        };

                        instruction
                    }
                    Operation::Br {
                        nzp: (n, z, p),
                        pc_offset,
                    } => {
                        let mut nzp = ((*n as u16) << 2) | ((*z as u16) << 1) | (*p as u16);
                        if nzp == 0 {
                            nzp = 0b111;
                        }

                        let loc = resolve_location_relative(&pgm, pc_offset, line.location)?;
                        (Opcode::Br.to_int() << 12) | (nzp << 9) | truncate::<9>(loc)?
                    }
                    Operation::Jmp { base_r } => (Opcode::Jmp.to_int() << 12) | (base_r.0 << 6),
                    Operation::Jsr { pc_offset } => {
                        let loc = resolve_location_relative(&pgm, pc_offset, line.location)?;
                        (Opcode::Jsr.to_int() << 12) | (1 << 11) | truncate::<11>(loc)?
                    }
                    Operation::Jsrr { base_r } => {
                        // don't you just hate it when you run out of instructions in your encoding
                        (Opcode::Jsr.to_int() << 12) | (base_r.0 << 6)
                    }
                    Operation::Ld { dr: reg, pc_offset }
                    | Operation::Ldi { dr: reg, pc_offset }
                    | Operation::Lea { dr: reg, pc_offset }
                    | Operation::St { sr: reg, pc_offset }
                    | Operation::Sti { sr: reg, pc_offset } => {
                        let op = match operation {
                            Operation::Ld { .. } => Opcode::Ld.to_int(),
                            Operation::Ldi { .. } => Opcode::Ldi.to_int(),
                            Operation::Lea { .. } => Opcode::Lea.to_int(),
                            Operation::St { .. } => Opcode::St.to_int(),
                            Operation::Sti { .. } => Opcode::Sti.to_int(),
                            _ => unreachable!(),
                        };
                        let loc = resolve_location_relative(&pgm, pc_offset, line.location)?;
                        (op << 12) | (reg.0 << 9) | truncate::<9>(loc)?
                    }
                    Operation::Ldr {
                        dr: reg,
                        base_r,
                        offset,
                    }
                    | Operation::Str {
                        sr: reg,
                        base_r,
                        offset,
                    } => {
                        let op = match operation {
                            Operation::Ldr { .. } => Opcode::Ldr.to_int(),
                            Operation::Str { .. } => Opcode::Str.to_int(),
                            _ => unreachable!(),
                        };
                        (op << 12) | (reg.0 << 9) | (base_r.0 << 6) | truncate::<6>(*offset)?
                    }
                    Operation::Not { dr, sr } => {
                        (Opcode::Not.to_int() << 12) | (dr.0 << 9) | (sr.0 << 6) | 0b111111
                    }
                    Operation::Ret => (Opcode::Jmp.to_int() << 12) | (7 << 6),
                    Operation::Rti => Opcode::Rti.to_int() << 12,
                    Operation::Trap { vector } => {
                        (Opcode::Trap.to_int() << 12) | truncate::<8>(*vector)?
                    }
                };

                machine_code.push(instruction);
            }

            Instruction::Trap(trap) => {
                let vector: u16 = match trap {
                    Trap::Getc => 0x20,
                    Trap::Out => 0x21,
                    Trap::Puts => 0x22,
                    Trap::In => 0x23,
                    Trap::Putsp => 0x24,
                    Trap::Halt => 0x25,
                };
                machine_code.push((Opcode::Trap.to_int() << 12) | vector);
            }
        }
    }

    Ok(machine_code)
}
