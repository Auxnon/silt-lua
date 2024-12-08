use std::vec;

use gc_arena::{Collect, Gc};
use crate::{code::OpCode, error::Location, value::Value};

// TODO benchmark/compare to using a manually resized array
#[derive(Default, Collect)]
#[collect(no_drop)]
pub struct Chunk<'chnk> {
    pub code: Vec<OpCode>,
    constants: Vec<Value<'chnk>>, //TODO VALUE ARRAY typedef faster?
    locations: Vec<(usize, usize)>,
    valid: bool,
}

impl<'chnk> Chunk<'chnk> {
    pub fn new() -> Self {
        Self {
            code: vec![],
            constants: vec![],
            locations: vec![],
            valid: true,
        }
    }
    // capacity < 8 ? 8: capacity*2

    pub fn write_code(&mut self, byte: OpCode, location: Location) -> usize {
        // TODO https://shnatsel.medium.com/how-to-avoid-bounds-checks-in-rust-without-unsafe-f65e618b4c1e
        self.code.push(byte);
        self.locations.push(location);
        self.code.len() - 1
    }

    pub fn read_last_code(&self) -> &OpCode {
        self.code.last().unwrap()
    }

    pub fn write_constant(&mut self, value: Value<'chnk>) -> usize {
        self.constants.push(value);
        // TODO limit to u8
        self.constants.len() - 1
    }

    // TODO lets change to a hashmap, cant see an advantage not to so far
    /** for global identifiers we attempt to resolve to an existing global variable if it exists and return that index */
    pub fn write_identifier(&mut self, identifier: Box<String>) -> usize {
        match self.constants.iter().enumerate().position(|(i, x)| {
            if let Value::String(s) = x {
                s == &identifier
            } else {
                false
            }
        }) {
            Some(i) => i,
            None => self.write_constant(Value::String(identifier)),
        }
    }

    pub fn write_value(&mut self, value: Value<'chnk>, location: Location) {
        let u = self.write_constant(value);
        self.write_code(OpCode::CONSTANT { constant: u as u8 }, location);
    }

    pub fn get_constant(&self, index: u8) -> &Value<'chnk> {
        &self.constants[index as usize]
    }

    pub fn copy_constant(&self, index: u8) -> Value<'chnk> {
        self.constants[index as usize].clone()
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    // fn add_constant(&mut self, value: Value) -> usize {
    //     self.constants.write_value(value)
    // }
    pub fn print_constants(&self) {
        println!("constants: {}", self.constants.len());
        self.constants.iter().for_each(|c| {
            print!("  {},", c);
        });
        println!();
    }

    pub fn print_chunk(&self, name: Option<String>) {
        match name {
            Some(n) => println!("=== Chunk ({}) ===", n),
            None => println!("=== Root Chunk ==="),
        }
        println!("code chunk: {}", self.code.len());
        let mut last = 0;
        for (i, c) in self.code.iter().enumerate() {
            let l = self.locations[i];
            if last == l.0 {
                print!("   ");
            } else {
                print!("{:03}", l.0);
            }
            // println!("{} {}:{}", c, l.0, l.1);
            let constant = match c {
                OpCode::CONSTANT { constant }
                | OpCode::DEFINE_GLOBAL { constant }
                | OpCode::GET_GLOBAL { constant }
                | OpCode::DEFINE_LOCAL { constant } => {
                    format!("({})", &self.constants[*constant as usize])
                }
                OpCode::GET_LOCAL { index } | OpCode::SET_LOCAL { index } => {
                    format!("(${})", index)
                }
                _ => String::new(),
            };

            println!(":{:02} | {} {}", l.1, c, constant);
            last = l.0;
        }
        println!("constants: {}", self.constants.len());
        self.constants.iter().for_each(|c| {
            print!("  {},", c);
        });
        println!();
        self.constants.iter().for_each(|c| {
            if let Value::Function(f) = c {
                f.chunk.print_chunk(match &f.name {
                    Some(n) => Some(n.clone()),
                    None => Some("anon-fn".to_string()),
                });
            }
        });
    }

    pub fn free(&mut self) {
        self.code.clear();
        self.constants.clear();
        self.locations.clear();
    }
}
