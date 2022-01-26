use alic3::asm_parser::Parser;
use std::{env::args, fs::File, io::Read};

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 2 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let mut asm = File::open(&args[1])?;
    let mut asm_str = String::new();
    asm.read_to_string(&mut asm_str)?;

    let program = Parser::parse(&asm_str)?;
    for line in program.lines {
        println!("{:?}", line);
    }

    Ok(())
}
