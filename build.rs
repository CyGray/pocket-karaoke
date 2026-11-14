fn main() {
    println!("cargo:rerun-if-changed=ui/app.slint");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        slint_build::compile_with_config(
            "ui/app.slint",
            slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
        )
        .expect("Slint UI must compile for the Windows target");
    }
}
