// #![no_main]
// use boa_engine::Context;

// #[no_mangle]
// pub extern "C" fn flow_init() {
//     let js_code = "console.log('Hello World from a JS code string!')";
//
//     // Instantiate the execution context
//     let mut context = Context::default();
//
//     // Parse the source code
//     match context.eval(js_code) {
//         Ok(res) => {
//             println!(
//                 "{}",
//                 res.to_string(&mut context).unwrap().to_std_string_escaped()
//             );
//         }
//         Err(e) => {
//             // Pretty print the error
//             eprintln!("Uncaught {e}");
//         }
//     };
// }

fn main() {
    println!("Hello world!");
    let mut x = 0;
    for i in 0..10 {
        x += i;
        println!("Now x is {}!", x);
    }
}
