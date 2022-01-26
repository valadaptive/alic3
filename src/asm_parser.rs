use std::collections::HashMap;

use anyhow::{anyhow, Error, Result};
use logos::{Lexer, Logos};

fn num<'a>(lex: &mut Lexer<'a, Token<'a>>) -> Result<i64> {
    let slice = lex.slice();
    Ok(if let Some((_, n)) = slice.split_once('x') {
        // Hex number--marked with an X with the digits to the right.
        // The negative sign comes after the digits so this works.
        i64::from_str_radix(n, 16)?
    } else {
        slice.trim_start_matches('#').parse::<i64>()?
    })
}

fn register<'a>(lex: &mut Lexer<'a, Token<'a>>) -> Option<u16> {
    let slice = lex.slice();
    slice.trim_start_matches(['R', 'r']).parse::<u16>().ok()
}

fn brnzp<'a>(lex: &mut Lexer<'a, Token<'a>>) -> (bool, bool, bool) {
    let slice = lex.slice();
    (
        slice.contains('n'),
        slice.contains('z'),
        slice.contains('p'),
    )
}

#[derive(Logos, Debug, PartialEq, Copy, Clone)]
enum Token<'a> {
    #[token(".orig", ignore(case))]
    Orig,
    #[token(".fill", ignore(case))]
    Fill,
    #[token(".blkw", ignore(case))]
    Blkw,
    #[token(".stringz", ignore(case))]
    Stringz,
    #[token(".end", ignore(case))]
    End,

    #[token("add", ignore(case))]
    Add,
    #[token("and", ignore(case))]
    And,
    #[regex("brn?z?p?", callback = brnzp, ignore(case))]
    Br((bool, bool, bool)),
    #[token("jmp", ignore(case))]
    Jmp,
    #[token("jsr", ignore(case))]
    Jsr,
    #[token("jsrr", ignore(case))]
    Jsrr,
    #[token("ldi", ignore(case))]
    Ldi,
    #[token("ldr", ignore(case))]
    Ldr,
    #[token("ld", ignore(case))]
    Ld,
    #[token("lea", ignore(case))]
    Lea,
    #[token("not", ignore(case))]
    Not,
    #[token("ret", ignore(case))]
    Ret,
    #[token("rti", ignore(case))]
    Rti,
    #[token("sti", ignore(case))]
    Sti,
    #[token("str", ignore(case))]
    Str,
    #[token("st", ignore(case))]
    St,
    #[token("trap", ignore(case))]
    Trap,

    #[token("getc", ignore(case))]
    Getc,
    #[token("halt", ignore(case))]
    Halt,
    #[token("in", ignore(case))]
    In,
    #[token("out", ignore(case))]
    Out,
    #[token("puts", ignore(case))]
    Puts,
    #[token("putsp", ignore(case))]
    Putsp,

    #[regex(r"(0?x-?[0-9a-fA-F]+)|(#?-?[0-9]+)", priority = 2, callback = num)]
    Number(i64),

    #[regex(r"[a-zA-Z_][a-zA-Z_0-9]*")]
    Label(&'a str),

    #[regex(r#""[^\\\n"]*(\\.[^\\\n"]*)*""#)]
    String(&'a str),

    #[regex(r"[rR][0-7]", callback = register)]
    Register(u16),

    #[regex(r",")]
    Separator,

    #[regex(r";.*", logos::skip)]
    Comment,

    #[regex(r"[ \t\r\n]+", logos::skip)]
    Whitespace,

    #[error]
    Error,
}

#[derive(Debug)]
pub struct Register(pub u16);

#[derive(Debug)]
pub enum RegOrImm {
    Register(Register),
    Immediate(u16),
}

#[derive(Debug)]
pub enum Location<'a> {
    Literal(u16),
    Label(&'a str),
}

#[derive(Debug)]
pub enum Operation<'a> {
    Add {
        sr1: Register,
        sr2: RegOrImm,
        dr: Register,
    },
    And {
        sr1: Register,
        sr2: RegOrImm,
        dr: Register,
    },
    Br {
        nzp: (bool, bool, bool),
        pc_offset: Location<'a>,
    },
    Jmp {
        base_r: Register,
    },
    Jsr {
        pc_offset: Location<'a>,
    },
    Jsrr {
        base_r: Register,
    },
    Ld {
        dr: Register,
        pc_offset: Location<'a>,
    },
    Ldi {
        dr: Register,
        pc_offset: Location<'a>,
    },
    Ldr {
        dr: Register,
        base_r: Register,
        offset: u16,
    },
    Lea {
        dr: Register,
        pc_offset: Location<'a>,
    },
    Not {
        dr: Register,
        sr: Register,
    },
    Ret,
    Rti,
    St {
        sr: Register,
        pc_offset: Location<'a>,
    },
    Sti {
        sr: Register,
        pc_offset: Location<'a>,
    },
    Str {
        sr: Register,
        base_r: Register,
        offset: u16,
    },
    Trap {
        vector: u16,
    },
}

#[derive(Debug)]
pub enum PseudoOp<'a> {
    Orig(u16),
    Fill(Location<'a>),
    Blkw(u16),
    Stringz(Vec<u8>),
    End,
}

#[derive(Debug)]
pub enum Trap {
    Getc,
    Halt,
    In,
    Out,
    Puts,
    Putsp,
}

#[derive(Debug)]
pub enum Instruction<'a> {
    Operation(Operation<'a>),
    PseudoOp(PseudoOp<'a>),
    Trap(Trap),
}

#[derive(Debug)]
pub struct CodeLine<'a> {
    pub label: Option<&'a str>,
    pub instruction: Instruction<'a>,
    pub location: u16,
}

pub struct Program<'a> {
    pub origin: u16,
    pub lines: Vec<CodeLine<'a>>,
    pub labels: HashMap<&'a str, u16>,
}

struct Scanner<'a> {
    next_token: Option<Token<'a>>,
    lexer: Lexer<'a, Token<'a>>,
}

impl<'a> Scanner<'a> {
    fn new(mut lexer: Lexer<'a, Token<'a>>) -> Self {
        let next_token = lexer.next();
        Self { next_token, lexer }
    }
    fn peek(&self) -> Option<Token<'a>> {
        self.next_token
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let old_next = self.next_token;
        self.next_token = self.lexer.next();
        old_next
    }
}

pub struct Parser<'a> {
    scanner: Scanner<'a>,
    location_cursor: u16,
    labels: HashMap<&'a str, u16>,
}

impl<'a> Parser<'a> {
    fn parse_separator(&mut self) -> bool {
        if let Some(Token::Separator) = self.scanner.peek() {
            self.scanner.next();
            return true;
        }
        false
    }
    fn parse_number(&mut self) -> Result<Option<u16>> {
        match self.scanner.peek() {
            Some(Token::Number(n)) => {
                const MIN_16: i64 = i16::MIN as i64;
                const MAX_16: i64 = u16::MAX as i64;
                match n {
                    MIN_16..=MAX_16 => {
                        self.scanner.next();
                        Ok(Some(n as u16))
                    }
                    _ => Err(anyhow!("{} cannot fit in 16 bits", n)),
                }
            }
            _ => Ok(None),
        }
    }
    fn parse_string(&mut self) -> Result<Option<Vec<u8>>> {
        match self.scanner.peek() {
            Some(Token::String(s)) => {
                let mut chars = Vec::<u8>::with_capacity(s.len());
                let mut prev_was_escaped = false;
                for char in s.as_bytes()[1..s.len() - 1].iter() {
                    if *char == b'\\' && !prev_was_escaped {
                        // eat the backslash regardless of whether the escape sequence is valid
                        prev_was_escaped = true;
                    } else {
                        if prev_was_escaped {
                            match *char {
                                b'a' => chars.push(b'\x07'),
                                b'b' => chars.push(b'\x08'),
                                b'f' => chars.push(b'\x0C'),
                                b'n' => chars.push(b'\n'),
                                b'r' => chars.push(b'\r'),
                                b't' => chars.push(b'\t'),
                                b'v' => chars.push(b'\x0B'),
                                b'"' => chars.push(b'"'),
                                b'\\' => chars.push(b'\\'),
                                _ => chars.push(*char),
                            };
                            prev_was_escaped = false;
                        } else {
                            chars.push(*char);
                        }
                    }
                }
                chars.push(b'\x00');
                if chars.len() >= 0xffff {
                    return Err(anyhow!("String does not fit in memory"));
                }
                chars.shrink_to_fit();

                self.scanner.next();
                Ok(Some(chars))
            }
            _ => Ok(None),
        }
    }

    fn parse_label(&mut self) -> Option<&'a str> {
        match self.scanner.peek() {
            Some(Token::Label(label_value)) => {
                self.scanner.next();
                Some(label_value)
            }
            _ => None,
        }
    }

    fn parse_register(&mut self) -> Option<Register> {
        match self.scanner.peek() {
            Some(Token::Register(r)) => {
                self.scanner.next();
                Some(Register(r))
            }
            _ => None,
        }
    }

    fn parse_location(&mut self) -> Result<Option<Location<'a>>> {
        if let Some(n) = self.parse_number()? {
            Ok(Some(Location::Literal(n)))
        } else {
            let label = self.parse_label();
            match label {
                Some(label) => Ok(Some(Location::Label(label))),
                None => Ok(None),
            }
        }
    }

    fn parse_instruction(&mut self) -> Result<Option<Instruction<'a>>> {
        let token = self.scanner.peek();
        match token {
            Some(Token::Add | Token::And) => {
                self.scanner.next();
                let dr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let sr1 = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let sr2 = if let Some(register) = self.parse_register() {
                    RegOrImm::Register(register)
                } else {
                    RegOrImm::Immediate(self.parse_number()?.ok_or_else(|| {
                        anyhow!("Expected register or number, got {:?}", self.scanner.peek())
                    })?)
                };
                let operation = match token {
                    Some(Token::Add) => Operation::Add { dr, sr1, sr2 },
                    Some(Token::And) => Operation::And { dr, sr1, sr2 },
                    _ => unreachable!(),
                };
                Ok(Some(Instruction::Operation(operation)))
            }
            Some(Token::Br(nzp)) => {
                self.scanner.next();
                let pc_offset = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number or label, got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Br {
                    nzp,
                    pc_offset,
                })))
            }
            Some(Token::Jmp) => {
                self.scanner.next();
                let base_r = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                Ok(Some(Instruction::Operation(Operation::Jmp { base_r })))
            }
            Some(Token::Jsr) => {
                self.scanner.next();
                let pc_offset = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number or label, got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Jsr { pc_offset })))
            }
            Some(Token::Jsrr) => {
                self.scanner.next();
                let base_r = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                Ok(Some(Instruction::Operation(Operation::Jsrr { base_r })))
            }
            Some(Token::Ld | Token::Ldi) => {
                self.scanner.next();
                let dr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let pc_offset = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number or label, got {:?}", self.scanner.peek())
                })?;
                let operation = match token {
                    Some(Token::Ld) => Operation::Ld { dr, pc_offset },
                    Some(Token::Ldi) => Operation::Ldi { dr, pc_offset },
                    _ => unreachable!(),
                };
                Ok(Some(Instruction::Operation(operation)))
            }
            Some(Token::Ldr) => {
                self.scanner.next();
                let dr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let base_r = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let offset = self.parse_number()?.ok_or_else(|| {
                    anyhow!("Expected number (Ldr), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Ldr {
                    dr,
                    base_r,
                    offset,
                })))
            }
            Some(Token::Lea) => {
                self.scanner.next();
                let dr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let pc_offset = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number or label, got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Lea {
                    dr,
                    pc_offset,
                })))
            }
            Some(Token::Not) => {
                self.scanner.next();
                let dr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let sr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                Ok(Some(Instruction::Operation(Operation::Not { dr, sr })))
            }
            Some(Token::Ret) => {
                self.scanner.next();
                Ok(Some(Instruction::Operation(Operation::Ret)))
            }
            Some(Token::Rti) => {
                self.scanner.next();
                Ok(Some(Instruction::Operation(Operation::Rti)))
            }
            Some(Token::St | Token::Sti) => {
                self.scanner.next();
                let sr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let pc_offset = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number or label, got {:?}", self.scanner.peek())
                })?;
                let operation = match token {
                    Some(Token::St) => Operation::St { sr, pc_offset },
                    Some(Token::Sti) => Operation::Sti { sr, pc_offset },
                    _ => unreachable!(),
                };
                Ok(Some(Instruction::Operation(operation)))
            }
            Some(Token::Str) => {
                self.scanner.next();
                let sr = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let base_r = self
                    .parse_register()
                    .ok_or_else(|| anyhow!("Expected register, got {:?}", self.scanner.peek()))?;
                let _ = self.parse_separator();
                let offset = self.parse_number()?.ok_or_else(|| {
                    anyhow!("Expected number (Str), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Str {
                    sr,
                    base_r,
                    offset,
                })))
            }
            Some(Token::Trap) => {
                self.scanner.next();
                let vector = self.parse_number()?.ok_or_else(|| {
                    anyhow!("Expected number (Trap), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::Operation(Operation::Trap { vector })))
            }

            Some(Token::Orig) => {
                self.scanner.next();
                let location = self.parse_number()?.ok_or_else(|| {
                    anyhow!("Expected number (Orig), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::PseudoOp(PseudoOp::Orig(location))))
            }
            Some(Token::Fill) => {
                self.scanner.next();
                let location = self.parse_location()?.ok_or_else(|| {
                    anyhow!("Expected number (Fill), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::PseudoOp(PseudoOp::Fill(location))))
            }
            Some(Token::Blkw) => {
                self.scanner.next();
                let size = self.parse_number()?.ok_or_else(|| {
                    anyhow!("Expected number (Blkw), got {:?}", self.scanner.peek())
                })?;
                Ok(Some(Instruction::PseudoOp(PseudoOp::Blkw(size))))
            }
            Some(Token::Stringz) => {
                self.scanner.next();
                let string = self
                    .parse_string()?
                    .ok_or_else(|| anyhow!("Expected string, got {:?}", self.scanner.peek()))?;
                Ok(Some(Instruction::PseudoOp(PseudoOp::Stringz(string))))
            }
            Some(Token::End) => {
                self.scanner.next();
                Ok(Some(Instruction::PseudoOp(PseudoOp::End)))
            }

            Some(Token::Getc) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::Getc)))
            }
            Some(Token::Halt) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::Halt)))
            }
            Some(Token::In) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::In)))
            }
            Some(Token::Out) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::Out)))
            }
            Some(Token::Puts) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::Puts)))
            }
            Some(Token::Putsp) => {
                self.scanner.next();
                Ok(Some(Instruction::Trap(Trap::Putsp)))
            }

            _ => Ok(None),
        }
    }

    fn parse_code_line(&mut self) -> Result<CodeLine<'a>> {
        let label = self.parse_label();
        let instruction = self
            .parse_instruction()?
            .ok_or_else(|| anyhow!("Expected instruction, got {:?}", self.scanner.peek()))?;

        Ok(CodeLine {
            label,
            instruction,
            location: self.location_cursor,
        })
    }

    fn parse_program(&mut self) -> Result<(u16, Vec<CodeLine<'a>>), Error> {
        match self.parse_instruction()? {
            Some(Instruction::PseudoOp(PseudoOp::Orig(origin))) => {
                self.location_cursor = origin;
                let mut lines = Vec::<CodeLine>::new();
                loop {
                    let line = self.parse_code_line()?;
                    if let Instruction::PseudoOp(PseudoOp::End) = line.instruction {
                        break;
                    }

                    // Keep track of instructions' locations in memory
                    let location_increment = match &line.instruction {
                        Instruction::PseudoOp(PseudoOp::Blkw(n)) => *n,
                        Instruction::PseudoOp(PseudoOp::Stringz(string)) => string.len() as u16,
                        _ => 1,
                    };
                    self.location_cursor = self
                        .location_cursor
                        .checked_add(location_increment)
                        .ok_or_else(|| anyhow!("Program goes past the end of memory"))?;
                    if self.location_cursor >= 0xfe00 {
                        return Err(anyhow!("Program goes into device register space"));
                    }

                    if let Some(label) = line.label {
                        self.labels.insert(label, line.location);
                    }

                    lines.push(line);
                }
                Ok((origin, lines))
            }
            _ => Err(anyhow!("Expected .ORIG")),
        }
    }

    pub fn parse(string: &'a str) -> Result<Program<'a>, Error> {
        let lexer = Token::lexer(string);

        let mut parser = Self {
            scanner: Scanner::new(lexer),
            location_cursor: 0,
            labels: HashMap::new(),
        };

        let (origin, lines) = parser.parse_program()?;
        Ok(Program {
            origin,
            lines,
            labels: parser.labels,
        })
    }
}
