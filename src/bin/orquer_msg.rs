fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let current_exe = std::env::current_exe().unwrap_or_else(|_| "orquer-msg".into());
    let orquer_exe = current_exe.with_file_name(if cfg!(windows) {
        "orquer.exe"
    } else {
        "orquer"
    });
    let bin = if orquer_exe.exists() {
        orquer_exe
    } else {
        "orquer".into()
    };
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("msg");
    cmd.args(args);
    let status = match cmd.status() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to execute orquer: {err}");
            std::process::exit(1);
        }
    };
    std::process::exit(status.code().unwrap_or(1));
}
