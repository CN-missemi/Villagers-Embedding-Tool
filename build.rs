fn main() {
    #[cfg(target_env = "msvc")]
    {
        let libs_to_link = [
            "crypt32", "Setupapi", "winmm", "Imm32", "Version", "mfplat", "strmiids", "mfuuid",
            "Vfw32", "OleAut32", "Shlwapi",
        ];
        for c in libs_to_link.iter() {
            println!("cargo:rustc-link-lib={}", c);
        }
    }
}
