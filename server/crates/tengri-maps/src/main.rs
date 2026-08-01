use std::env;
use std::path::PathBuf;
use std::process;

mod export;

fn main() {
    let mut args = env::args_os();
    let program = args
        .next()
        .and_then(|arg| PathBuf::from(arg).file_name().map(|name| name.to_owned()))
        .and_then(|name| name.into_string().ok())
        .unwrap_or_else(|| "tengri-maps".to_owned());

    let Some(command_or_path) = args.next() else {
        export::print_usage(&program);
        process::exit(2);
    };

    if command_or_path == "--help" || command_or_path == "-h" {
        export::print_usage(&program);
        return;
    }

    if command_or_path == "export-tree" {
        export::export_tree(&program, args);
        return;
    }

    if args.next().is_some() {
        export::print_usage(&program);
        process::exit(2);
    }

    export::print_usage(&program);
}
