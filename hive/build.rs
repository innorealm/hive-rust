fn main() {
    // _build_thrift();
}

fn _build_thrift() {
    use std::process::Command;

    Command::new("thrift")
        .args([
            "-out",
            "src/service/rpc/thrift",
            "--gen",
            "rs",
            "thrift/TCLIService.thrift",
        ])
        .status()
        .unwrap();

    println!("cargo:rerun-if-changed=thrift/TCLIService.thrift");
}
