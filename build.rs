include!("src/glyph.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/glyph.rs");

    let folder = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let file = folder.join("icon.ico");
    if std::fs::write(&file, ico()).is_err() {
        return;
    }

    if cfg!(target_os = "windows")
        && let Some(path) = file.to_str()
    {
        let _ = winresource::WindowsResource::new().set_icon(path).compile();
    }
}

fn ico() -> Vec<u8> {
    let side = SIDE as usize;
    let mut pixels = vec![0u8; side * side * 4];
    for y in 0..SIDE {
        for x in 0..SIDE {
            if !lit(x, y) {
                continue;
            }
            let flipped = (SIDE - 1 - y) as usize;
            let at = (flipped * side + x as usize) * 4;
            pixels[at] = INK[2];
            pixels[at + 1] = INK[1];
            pixels[at + 2] = INK[0];
            pixels[at + 3] = 0xff;
        }
    }
    let mask = vec![0u8; side * side / 8];

    let mut out = Vec::new();
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.push(SIDE as u8);
    out.push(SIDE as u8);
    out.extend([0u8, 0]);
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend(((40 + pixels.len() + mask.len()) as u32).to_le_bytes());
    out.extend(22u32.to_le_bytes());

    out.extend(40u32.to_le_bytes());
    out.extend((SIDE as i32).to_le_bytes());
    out.extend((SIDE as i32 * 2).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend((pixels.len() as u32).to_le_bytes());
    out.extend([0u8; 16]);

    out.extend(pixels);
    out.extend(mask);
    out
}
