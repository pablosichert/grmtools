use lrlex::lrlex_mod;
use lrpar::lrpar_mod;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(string: &str);
}

lrlex_mod!(calc_l);
lrpar_mod!(calc_y);

#[wasm_bindgen]
pub fn parse(input: &str) {
    log(&format!("Parsing input: {}", input));

    let lexerdef = calc_l::lexerdef();

    let lexer = lexerdef.lexer(input);

    let (res, errs) = calc_y::parse(&lexer);

    for e in errs {
        println!("{}", e.pp(&lexer, &calc_y::token_epp));
    }

    match res {
        Some(Ok(r)) => log(&format!("Result: {}", r)),
        _ => log(&format!("Unable to evaluate expression."))
    }
}
