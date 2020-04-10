use move_lang::cfgir as libra_cfgir;
use move_lang::errors::Errors;
use move_lang::expansion as libra_expansion;
use move_lang::hlir as libra_hlir;
use move_lang::naming as libra_naming;
use move_lang::parser as libra_parser;
use move_lang::parser::ast::FileDefinition;
use move_lang::shared::Address;
use move_lang::typing as libra_typing;

pub fn check_parsed_program(
    main_file: FileDefinition,
    dependencies: Vec<FileDefinition>,
    sender_opt: Option<Address>,
) -> Result<(), Errors> {
    let program = libra_parser::ast::Program {
        source_definitions: vec![main_file],
        lib_definitions: dependencies,
    };
    // expansion step
    // TODO: pretty sure stdlib could be excluded from expansion on every change (cached)
    let (e_program, errors) = libra_expansion::translate::program(program, sender_opt);
    if !errors.is_empty() {
        return Err(errors);
    }
    // naming step
    let (n_program, errors) = libra_naming::translate::program(e_program, errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    // typechecking step
    let (t_program, errors) = libra_typing::translate::program(n_program, errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    // reachability and liveness analysis
    let (hlir_program, errors) = libra_hlir::translate::program(t_program);
    if !errors.is_empty() {
        return Err(errors);
    }
    let (_, errors) = libra_cfgir::translate::program(errors, hlir_program);
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(())
}
