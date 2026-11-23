fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/undertow.rc");
        println!("cargo:rerun-if-changed=assets/undertow.ico");
        embed_resource::compile_for("assets/undertow.rc", ["undertow"], embed_resource::NONE)
            .manifest_required()
            .expect("Embed Undertow's Windows icon");
    }
}
