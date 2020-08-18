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

fn main() {
    generate_vector_asm();
    println!("cargo:rerun-if-changed=build.rs");
}
