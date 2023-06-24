use std::vec;

use crate::{code::OpCode, error::Location, value::Value};

// TODO benchmark/compare to using a manually resized array
#[derive(Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    constants: Vec<Value>, //TODO VALUE ARRAY typedef faster?
    locations: Vec<(usize, usize)>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: vec![],
            constants: vec![],
            locations: vec![],
        }
    }
    // capacity < 8 ? 8: capacity*2

    pub fn write_code(&mut self, byte: OpCode, location: Location) {
        // TODO https://shnatsel.medium.com/how-to-avoid-bounds-checks-in-rust-without-unsafe-f65e618b4c1e
        self.code.push(byte);
        self.locations.push(location);
    }

    pub fn write_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        // TODO limit to u8
        self.constants.len() - 1
    }

    pub fn write_value(&mut self, value: Value, location: Location) {
        let u = self.write_constant(value);
        self.write_code(OpCode::CONSTANT { constant: u as u8 }, location);
    }

    pub fn get_constant(&self, index: u8) -> &Value {
        &self.constants[index as usize]
    }

    // fn add_constant(&mut self, value: Value) -> usize {
    //     self.constants.write_value(value)
    // }
    pub fn print_chunk(&self) {
        println!("=== Chunk ===");
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
            println!(":{:02} | {}", l.1, c);
            last = l.0;
        }
    }

    pub fn free(&mut self) {
        self.code.clear();
        self.constants.clear();
        self.locations.clear();
    }
}
