fn main() {
    let shaders = [
        "main.vert",
        "main.frag",
        "post_effect.vert",
        "post_effect.frag",
    ];
    let src_dir = std::env::current_dir()
        .unwrap()
        .join("src")
        .join("vkapp")
        .join("shaders");
    let out_dir_str = std::env::var("OUT_DIR").unwrap();
    let out_dir = std::path::Path::new(&out_dir_str);
    for shader in shaders {
        println!("cargo::rerun-if-changed=src/{}", shader);
        let in_file = src_dir.join(shader);
        let out_file = out_dir.join(format!("{shader}.spv"));
        let mut cmd = std::process::Command::new("glslc");
        cmd.arg(in_file).arg("-g").arg("-o").arg(out_file);
        dbg!(&cmd);
        let success = cmd.spawn().unwrap().wait().unwrap().success();
        assert!(success);
    }
}
