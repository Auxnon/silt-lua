use core::panic;
use std::{rc::Rc, vec};

use silt_lua::silt::SiltLua;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    {
        // cli(source_in, &mut global);

        // match cli(source_in, &mut global) {
        //     value::Value::Nil => {}
        //     v => println!("{}", v),
        // }
        // println!(">");

        // loop {
        //     let mut line = String::new();
        //     if let Ok(_) = std::io::stdin().read_line(&mut line) {
        //         let s = line.trim();
        //         match cli(s, &mut global) {
        //             value::Value::Nil => {}
        //             v => println!("{}", v),
        //         }
        //         println!(">");
        //     }
        // }

        // println!("Hello, world!");
        // let source = r#"
        // abc
        // abc2
        // bc=5--hi there
        // a=1
        // b=2
        // c=a+b
        // d='hello'
        // e="world"
        // f=[[multi
        // line]]
        // print(c)
        // if a==1 then
        // a=2
        // end
        // print(a)
        // let source_in =
        // "#;
        // let source_in = r#"
        // a=1
        // while a < 10_000_00 do
        //     a = a + 1
        // end
        // print a
        // "#;

        //benchmark
        // let source_in = r#"
        // start=clock()
        // i=1
        // a="a"
        // while i < 100_000 do
        //     i = i +1
        //     a = a .. "1"
        // end
        // elapsed=clock()-start
        // print "done"
        // print elapsed
        // elapsed
        // "#;

        // func test
        // let source_in = r#"

        // n=1
        // function b(h) print 'b'..n n=h print 'c'..n end

        // function a() print 'e'..n  n=3 print 'f'..n b(10) print 'g'..n end

        // print 'a'..n
        // b(9)
        // print 'd'..n
        // a()
        // print 'h'.. n
        // "#;

        // thrice
        //     let source_in = r#"

        // function thrice(fn)
        // for i = 1,  1 do
        //     fn(i)
        // end
        // end

        // thrice(function(i) print("p "..i) end)
        // "#;

        //     let source_in = r#"
        //     function create_counter()
        //     local count = 0
        //     return function()
        //         count = count + 1
        //         print(count)
        //         return count
        //     end
        // end

        // local counter = create_counter()
        // counter()
        // counter()
        //     "#;
        // let source_in = r#"
        // function echo(n)
        // print("x"..n)
        // return 2
        // end

        // echo(2)
        // print("y"..2)

        // --print(echo(echo(1)+echo(2)) + echo(echo(4)+echo(5)) )
        // "#;
        // let source_in = r#" 4*-6*(3 * 7 + 5) + 2 * 9"#;

        //     let source_in = r#"
        //     do
        //     local a=1
        //     if true then
        //         local b=2
        //         print(a)
        //     end
        //     print(b)
        // end
        //     "#;

        //     let source_in = r#"
        // "#;

        // panic!("test");

        // fibonaci
        // let source_in = r#"
        // a=0
        // b=1
        // while a < 10000 do
        //     b+=temp
        //     print a
        //     temp=a
        //     a=b
        // end
        // "#;
        // let source_in = "local a= 'test'..'1' a='now' sprint a"; // sprint 1+8";
        // let source_in = "local a= -5+6";
    }
    let source_in = r#"!(5-4 > 3*2 == !nil)"#;
    let source_in = "!nil==!false";

    let source_in = r#"
    local a="test"
    local b="t"
    sprint b
     b="a"
     sprint b
    b=a.."working"
    sprint b
     "#;

    let source_in = r#"
     local a=2
     local b=1
     a*b=5
     sprint ab
     "#;
    let source_in = r#"
    do
    local a=1
    a=3+4
    sprint a
    end
    sprint a
    "#;
    let source_in = r#"
    do
    local a=3
    sprint a
    end
    sprint a
    "#;
    // REMEMBER 10,000,000 takes about ~6.47 seconds
    let source_in = r#"
    do
    local a=1
    while a<= 10_000_000 do
        a=a+1
    end
        sprint a
    end
    "#;

    let source_in = r#"
    "#;

    let source_in = r#"
        function fib(n)
            if n <= 1 then
            return n
            else
                return fib(n-1) + fib(n-2)
            end
        end
        -- return fib(21)

        for i = 1, 35 do
            sprint i..":"..fib(i)
        end
    "#;

    let source_in = r#"
    start=clock()
    return 1000*(clock() - start)
    "#;
    let source_in = r#"
    start=clock()
    i=1
    a="a"
    while i < 10 do
        i = i +1
        a = a .. "1"
    end
    elapsed=clock()-start
    print "done"
    print ("elapsed: "..elapsed)
    return elapsed
    "#;
    let source_in = r#"
    local d=5
    function sum()
    local a=1
    local b=2
    local c=3
    return a+b+c+d+8
    end

    return sum()
    "#;
    // load string from scripts/closure4.lua
    let file = if args.len() > 1 {
        std::fs::read_to_string(args[1].as_str()).unwrap()
    } else {
        std::fs::read_to_string("scripts/table.lua").unwrap()
    };
    let source_in = file.as_str();
    // let source_in = r#"
    // global g=2
    // function is1(n)
    //     if n==1 then
    //         return 'ye'
    //     else
    //         return 'naw'
    //     end
    // end
    // return is1(1)
    // "#;

    // let source_in = r#"
    // do
    // local a=5
    // sprint a
    // a=nil
    // sprint a
    // local b=5+6
    // 3+2
    // a=1+4 +b
    // sprint a
    // end
    // "#;
    // TODO should get about 100 million in a second for lua
    // let source_in = r#"
    // do
    // local a=1
    // for i=1, 10 do
    // a=i
    // end
    // sprint a
    // end
    // "#;

    // let mut compiler = Compiler::new();
    // let o = compiler.compile(source_in.to_string());

    // #[cfg(feature = "dev-out")]
    // o.chunk.print_chunk(None);
    // compiler.print_errors();

    let mut vm = SiltLua::new();
    vm.load_standard_library();
    match vm.run(source_in) {
        Ok(o) => {
            println!("-----------------");
            println!(">> {}", o);
        }
        Err(e) => {
            e.iter().for_each(|e| println!("!!Err: {}", e));
        }
    }
}

// fn cli(source: &str, global: &mut environment::Environment) -> value::Value {
//     println!("-----------------");
//     let mut lexer = lexer::Lexer::new(source.to_owned());
//     let mut t: Vec<TokenResult> = lexer.collect();
//     let mut erronious = false;
//     let mut tokens = vec![];
//     let mut locations = vec![];
//     t.drain(..).enumerate().for_each(|(i, res)| match res {
//         Ok(t) => {
//             let p = t.1;
//             println!("|{}:{}| {}", p.0, p.1, t.0);

//             tokens.push(t.0);
//             locations.push(t.1);
//         }
//         Err(e) => {
//             erronious = true;
//             println!("|!! {}", e)
//         }
//     });

//     if !erronious {
//         let mut parser = crate::parser::parser::Parser::new(tokens, locations, global);
//         println!("|----------------");
//         let mut statements = parser.parse();
//         statements
//             .iter()
//             .enumerate()
//             .for_each(|(i, e)| println!("| {} {}", i, e));
//         // println!("{}", exp);
//         // let val = parser.evaluate(exp);
//         let err = parser.get_errors();
//         if err.len() > 0 {
//             println!("|----------------");
//             println!("Parse Errors:");
//             err.iter().for_each(|e| println!("{}", e));
//             println!("-----------------");
//         } else {
//             println!("-----------------");
//             let mut resolver = crate::resolver::Resolver::new();
//             resolver.process(&mut statements);
//             let res = crate::interpreter::execute(global, &statements);
//             match res {
//                 Ok(v) => {
//                     println!("");
//                     return v;
//                 }
//                 Err(e) => {
//                     println!("| Runtime Errors:");
//                     println!("| {}", e);
//                     println!("-----------------");
//                 }
//             }
//         }
//     }

//     println!("");
//     value::Value::Nil
// }
