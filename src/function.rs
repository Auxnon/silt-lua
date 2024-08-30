use std::{cell::RefCell, fmt::Display, rc::Rc};

use gc_arena::Collect;

use crate::{chunk::Chunk, code::OpCode, lua::Lua, value::Value};

/////////////
///
pub struct CallFrame<'lua> {
    pub function: Rc<Closure<'lua>>, // pointer
    // ip: *const OpCode
    // pub base: usize,
    // pointer point sinto VM values stack
    pub stack_snapshot: usize,
    pub local_stack: *mut Value<'lua>,
    pub ip: *const OpCode,
}

impl<'frame> CallFrame<'frame> {
    pub fn new<'a>(function: Rc<Closure<'a>>, stack_snapshot: usize) -> Self
    where
        'a: 'frame,
        'frame: 'a,
    {
        let ip = function.function.chunk.code.as_ptr();
        Self {
            function,
            ip,
            local_stack: std::ptr::null_mut(),
            stack_snapshot,
        }
    }

    pub fn current_instruction(&self) -> &crate::code::OpCode {
        // &self.function.chunk.code[self.ip]
        unsafe { &*self.ip }
    }

    /** shift ip by 1 instruction */
    pub fn iterate(&mut self) {
        // self.ip += 1;
        self.ip = unsafe { self.ip.add(1) };
    }

    /** DANGER: does not shift ip, only returns instruction set in range past ip */
    pub fn get_next_n_codes(&self, n: usize) -> &[OpCode] {
        // &self.function.chunk.code[self.ip..self.ip + n]
        const SIZE: usize = std::mem::size_of::<OpCode>();
        unsafe { std::slice::from_raw_parts(self.ip.add(1), n * SIZE) }
    }

    /** move ip N instructions over */
    pub fn shift(&mut self, n: usize) {
        // self.ip += n;
        self.ip = unsafe { self.ip.add(n) };
    }

    pub fn set_val(&mut self, index: u8, value: Value<'frame>) {
        // self.stack[index as usize] = value;
        unsafe { *self.local_stack.add(index as usize) = value };
    }

    pub fn get_val(&self, index: u8) -> &Value<'frame> {
        // &self.stack[index as usize]
        // println!("get_val: {}", index);
        // println!("top: {}", unsafe { &*self.local_stack });
        unsafe { &*self.local_stack.add(index as usize) }
    }

    pub fn get_val_mut(&self, index: u8) -> &mut Value<'frame> {
        unsafe { &mut *self.local_stack.add(index as usize) }
    }

    pub fn print_local_stack(&self) {
        println!("local stack: {:?}", unsafe {
            std::slice::from_raw_parts(self.local_stack, 10)
        });
    }

    // pub fn push(&mut self, value: Value) {
    //     // TODO can we push to the stack by pointer? Or should we just push on a Vec?
    //     // *self.stack_top= value;

    //     // unsafe { *self.stack_top = value };
    //     // self.stack.push(value);
    //     // self.stack_top = self.stack.as_ptr().add(self.stack.len());

    //     // unsafe { *self.stack_top = value };
    //     // self.stack_top = unsafe { self.stack_top.add(1) };
    //     self.stack.push(value);
    // }

    // pub fn push(&mut self, value: Value) {
    //     println!("pushing: {}", value);
    //     // self.stack.push(value);
    //     self.stack = unsafe { self.stack.add(1) };
    //     unsafe { *self.stack = value };
    // }

    /** pop and return top of stack */
    // pub fn pop(&mut self) -> Value {
    //     // self.stack.pop().unwrap()
    //     // let d = self.stack.wrapping_add(1);
    //     // take value
    //     let v = unsafe { std::mem::replace(&mut *self.stack, Value::Nil) };
    //     self.stack = unsafe { self.stack.sub(1) };
    //     v

    //     // let o = unsafe { &*self.stack };
    //     // o
    // }

    /** pop N number of values from stack */
    // pub fn popn(&mut self, n: u8) {
    //     // self.stack.truncate(self.stack.len() - n as usize);
    //     self.stack = unsafe { self.stack.sub(n as usize) };
    // }

    /** take and replace with a Nil */
    pub fn take(&'frame mut self) -> &Value {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        let v = unsafe { &*self.local_stack };
        unsafe { *self.local_stack = Value::Nil };
        v
    }

    // TODO validate safety of this, compiler has to be solid af!
    pub fn forward(&mut self, offset: u16) {
        // self.ip += offset as usize;
        self.ip = unsafe { self.ip.add(offset as usize) };
    }

    pub fn rewind(&mut self, offset: u16) {
        // self.ip -= offset as usize;
        self.ip = unsafe { self.ip.sub(offset as usize) };
        // println!("rewind: {}", unsafe { &*self.ip });
    }
}
#[derive(Default, Collect)]
#[collect(no_drop)]
pub struct FunctionObject<'chnk> {
    pub is_script: bool,
    pub name: Option<String>,
    pub chunk: Chunk<'chnk>,
    pub upvalue_count: u8,
    // pub arity: usize,
}

impl<'chnk> FunctionObject<'chnk> {
    pub fn new(name: Option<String>, is_script: bool) -> Self {
        Self {
            name,
            is_script,
            chunk: Chunk::new(),
            upvalue_count: 0,
        }
    }
    pub fn set_chunk(&mut self, chunk: Chunk<'chnk>) {
        self.chunk = chunk;
    }
}

impl Display for FunctionObject<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_script {
            write!(
                f,
                "module={}",
                self.name.as_ref().unwrap_or(&"root".to_string())
            )
        } else {
            write!(
                f,
                "fn {}()",
                self.name.as_ref().unwrap_or(&"anonymous".to_string())
            )
        }
    }
}

// pub type NativeFunction<'lua> = &'static fn(&'lua mut Lua, Vec<Value>) -> Value<'lua>; // TODO should be Result<Value,SiltError> for runtime errors
// pub struct NativeObject {
//     name: &'static str,
//     pub function: NativeFunction<'static>,
// }

pub type NativeFunction<'lua> = fn(&mut Lua, Vec<Value>) -> Value<'lua>;

// impl NativeObject {
//     pub fn new(name: &'static str, function: NativeFunction) -> Self {
//         Self { name, function }
//     }
// }

pub struct Closure<'lua> {
    pub function: Rc<FunctionObject<'lua>>,
    pub upvalues: Vec<Rc<RefCell<UpValue<'lua>>>>,
}

impl<'chnk> Closure<'chnk> {
    pub fn new(
        function: Rc<FunctionObject<'chnk>>,
        upvalues: Vec<Rc<RefCell<UpValue<'chnk>>>>,
    ) -> Self {
        Self { function, upvalues }
    }
    pub fn print_upvalues(&self) {
        self.upvalues.iter().enumerate().for_each(|(i, f)| {
            println!("fn-up {}:{}", i, f.borrow());
        });
    }
}

pub struct UpValue<'lua> {
    // is_open: bool,
    // obj?
    pub index: u8,
    closed: Value<'lua>,
    pub location: *mut Value<'lua>,
    // pub value: *mut Value, // TODO oshould be a RC mutex of the value ideally
}
impl<'lua> UpValue<'lua> {
    pub fn new(index: u8, location: *mut Value<'lua>) -> Self {
        Self {
            index,
            closed: Value::Nil,
            location,
        }
    }

    pub fn set_value(&mut self, value: Value<'lua>) {
        unsafe { *self.location = value }
    }

    pub fn close_around(&mut self, value: Value<'lua>) {
        self.closed = value;
        self.location = &mut self.closed as *mut Value;
    }

    pub fn close(&mut self) {
        #[cfg(feature = "dev-out")]
        println!("closing: {}", unsafe { &*self.location });
        self.closed = unsafe { self.location.replace(Value::Nil) };
        #[cfg(feature = "dev-out")]
        println!("closed: {}", self.closed);
        self.location = &mut self.closed as *mut Value;
    }

    pub fn copy_value(&self) -> Value<'lua> {
        #[cfg(feature = "dev-out")]
        println!("copying: {}", unsafe { &*self.location });
        unsafe { (*self.location).clone() }
    }

    pub fn get_location(&self) -> *mut Value<'lua> {
        #[cfg(feature = "dev-out")]
        println!("getting location: {}", unsafe { &*self.location });
        self.location
    }

    // pub fn get(&self) -> &Value {
    //     unsafe { &*self.value }
    // }
    // pub fn set(&mut self, value: Value) {
    //     unsafe { *self.value = value };
    // }
}

impl Display for UpValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "⬆️{}x{}@{}",
            unsafe { &*self.location },
            self.closed,
            self.index
        )
    }
}
