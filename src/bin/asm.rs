use alic3::asm_parser::Parser;
use alic3::assembler::assemble;
use std::{
    env::args,
    fs::File,
    io::{Read, Write},
    path::Path,
};

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 2 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let in_path = Path::new(&args[1]);
    let mut out_path = in_path.to_path_buf();
    out_path.set_extension("obj");

    let mut asm = File::open(&args[1])?;
    let mut asm_str = String::new();
    asm.read_to_string(&mut asm_str)?;

    let program = Parser::parse(&asm_str).map_err(|err| {
        let start = err.span.start;
        let mut cursor_pos = start;
        let mut stop = false;
        let mut line_num: usize = 0;
        let err_line = asm_str
            .split('\n')
            .map_while(|line| {
                if stop {
                    return None;
                }
                if line.len() >= cursor_pos {
                    stop = true;
                } else {
                    cursor_pos -= line.len() + 1;
                }
                line_num += 1;
                Some(line.trim_end())
            })
            .last()
            .unwrap();
        println!("{}", err_line);
        println!("{}^", "-".repeat(cursor_pos));
        anyhow::anyhow!("Line {}: {}", line_num, err)
    })?;

    let machine_code = assemble(program)?;

    let mut out_file = File::create(out_path)?;
    let machine_code_bytes: Vec<u8> = machine_code
        .into_iter()
        .map(|instruction| [(instruction >> 8) as u8, (instruction & 0xff) as u8])
        .flatten()
        .collect();

    out_file.write(&machine_code_bytes)?;

    Ok(())
}
