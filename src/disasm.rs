use std::{env::args, fs::File, io::Read};

mod bit_twiddling;
mod opcode;

use crate::bit_twiddling::*;
use crate::opcode::*;

const MEMORY_SIZE: usize = u16::MAX as usize;

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 2 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let mut file = File::open(&args[1])?;
    let mut buf: [u8; MEMORY_SIZE * 2] = [0u8; MEMORY_SIZE * 2];
    let num_bytes = file.read(&mut buf)?;

    buf[0..num_bytes]
        .chunks(2)
        .enumerate()
        .for_each(|(i, chunk)| {
            let instruction = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
            let mut parts = Vec::<String>::new();
            let opcode = Opcode::try_from_int(get_bits::<12, 15>(instruction) as u8);
            if let Ok(opcode) = opcode {
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
                            parts.push(get_bits::<0, 4>(instruction).to_string());
                        } else {
                            parts.push(format!("R{}", get_bits::<0, 2>(instruction)));
                        }
                    }
                    _ => {
                        parts.push("Unknown opcode".to_string());
                        parts.push(get_bits::<12, 15>(instruction).to_string());
                    }
                }
            } else {
                // TODO
            }

            if (parts.len() > 0) {
                println!("{}", parts.join(" "))
            };
        });

    Ok(())
}
