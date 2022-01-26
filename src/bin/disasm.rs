use std::env::args;
use std::fs::File;
use std::io::prelude::*;

use byteorder::BigEndian;
use byteorder::ReadBytesExt;

use alic3::bit_twiddling::*;
use alic3::opcode::*;

fn disassemble_instruction(instruction: u16) -> String {
    let mut parts = Vec::<String>::new();
    let opcode = Opcode::from_int(get_bits::<12, 15>(instruction) as u8);
    match opcode {
        Opcode::Add | Opcode::And => {
            parts.push(
                match opcode {
                    Opcode::Add => "ADD",
                    Opcode::And => "AND",
                    _ => unreachable!(),
                }
                .to_string(),
            );

            parts.push(format!("R{}", get_bits::<9, 11>(instruction)));
            parts.push(format!("R{}", get_bits::<6, 8>(instruction)));

            if get_bits::<5, 5>(instruction) == 1 {
                parts.push(sign_extend::<5>(get_bits::<0, 4>(instruction) as i16).to_string());
            } else {
                parts.push(format!("R{}", get_bits::<0, 2>(instruction)));
            }
        }

        Opcode::Not => {
            parts.push("NOT".to_string());
            parts.push(format!("R{}", get_bits::<9, 11>(instruction)));
            parts.push(format!("R{}", get_bits::<6, 8>(instruction)));
        }

        Opcode::Br => {
            let nzp = get_bits::<9, 11>(instruction);
            let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);
            parts.push(format!(
                "BR{}{}{}",
                if nzp & 0b100 == 0 { "" } else { "n" },
                if nzp & 0b010 == 0 { "" } else { "z" },
                if nzp & 0b001 == 0 { "" } else { "p" },
            ));
            parts.push(format!("{:#06x}", pc_offset));
        }

        Opcode::Jmp => {
            let base_register = get_bits::<6, 8>(instruction);
            parts.push(if base_register == 7 { "RET" } else { "JMP" }.to_string());
            if base_register != 7 {
                parts.push(format!("R{}", base_register));
            }
        }

        Opcode::Jsr => {
            if get_bits::<11, 11>(instruction) == 1 {
                // JSR: PC-relative
                let pc_offset = sign_extend::<11>(get_bits::<0, 10>(instruction) as i16);
                parts.push("JSR".to_string());
                parts.push(format!("{:#06x}", pc_offset));
            } else {
                // JSRR: absolute
                parts.push("JSRR".to_string());
                parts.push(format!("R{}", get_bits::<6, 8>(instruction)));
            }
        }

        Opcode::Ld | Opcode::Ldi | Opcode::Lea | Opcode::St | Opcode::Sti => {
            let pc_offset = sign_extend::<9>(get_bits::<0, 8>(instruction) as i16);

            parts.push(
                match opcode {
                    // load effective address: just return the address
                    Opcode::Lea => "LEA",
                    // load direct
                    Opcode::Ld => "LD",
                    // load indirect
                    Opcode::Ldi => "LDI",
                    Opcode::St => "ST",
                    Opcode::Sti => "STI",
                    _ => unreachable!(),
                }
                .to_string(),
            );

            parts.push(format!("R{}", get_bits::<9, 11>(instruction)));

            parts.push(format!("{:#06x}", pc_offset));
        }

        Opcode::Ldr | Opcode::Str => {
            parts.push(
                match opcode {
                    Opcode::Ldr => "LDR",
                    Opcode::Str => "STR",
                    _ => unreachable!(),
                }
                .to_string(),
            );
            parts.push(format!("R{}", get_bits::<9, 11>(instruction)));
            parts.push(format!("R{}", get_bits::<6, 8>(instruction)));
            let offset = sign_extend::<6>(get_bits::<0, 5>(instruction) as i16);
            parts.push(format!("{:#06x}", offset));
        }

        Opcode::Rti => {
            parts.push("RTI".to_string());
        }

        Opcode::Reserved => {
            parts.push("[reserved]".to_string());
        }

        Opcode::Trap => {
            parts.push("TRAP".to_string());
            let trap_vector = get_bits::<0, 7>(instruction) as u8;
            parts.push(format!("{:#04x}", trap_vector));
        }
    }

    parts.join(" ")
}

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 2 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let mut pgm = File::open(&args[1])?;

    let mut buf = Vec::new();
    pgm.read_u16::<BigEndian>()?;
    pgm.read_to_end(&mut buf)?;
    buf.chunks(2).enumerate().for_each(|(i, chunk)| {
        let instruction = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
        println!(
            "{:#06x}: {:#06x} ({})",
            i,
            instruction,
            disassemble_instruction(instruction)
        );
    });

    Ok(())
}
