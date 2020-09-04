// Generates vector.S for trap.rs
fn generate_vector_asm() {
    let mut asm_string = ".globl alltraps\n".to_owned();
    for i in 0..256 {
        asm_string += &format!(
            ".globl vector0\nvector{}:\n  pushl $0\n  pushl ${}\n  jmp alltraps\n",
            i, i
        );
    }
    asm_string += ".data\n.globl VECTORS\nVECTORS:\n";
    for i in 0..256 {
        asm_string += &format!("  .long vector{}\n", i);
    }

    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("vectors.S");
    std::fs::write(&dest_path, asm_string).unwrap();
}

fn copy_init() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let out_dir = std::path::Path::new(&out_dir);

    let init_bin = out_dir
        .ancestors()
        .nth(6)
        .unwrap()
        .join("out")
        .join("init.bin");
    std::fs::copy(&init_bin, out_dir.join("init.bin")).unwrap();

    println!("cargo:rerun-if-changed={}", init_bin.to_str().unwrap());
}

fn main() {
    generate_vector_asm();
    copy_init();
    println!("cargo:rerun-if-changed=build.rs");
}
