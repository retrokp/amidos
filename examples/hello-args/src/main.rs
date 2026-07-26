//! Prints out command line arguments.

#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

amidos::startup_code!();
amidos::panic_handler_immediate_abort_minimal!();

fn amidos_main(dos: &mut amidos::Dos, args: amidos::MainArgs) -> i32 {
    let Some(mut output) = dos.output() else {
        // no output stream: most likely launched from Workbench
        return amidos::EXIT_CODE_ERROR;
    };
    // launched from CLI
    let _ = output.write_all(b"Command name: ");
    match args {
        amidos::MainArgs::Cli(cli) => {
            let _ = output.write_all(cli.command_name.to_bytes());
            let _ = output.write_all(c"\nArguments: ".to_bytes());
            let _ = output.write_all(cli.arguments.to_bytes());
            let _ = output.write_all(&[b'\n']);
        }
        amidos::MainArgs::Workbench(_wb) => {
            let _ = output.write_all(b"?? got workbench args.");
        }
    }
    let _ = output.write_all(b"Done.\n");
    amidos::EXIT_CODE_OK
}
