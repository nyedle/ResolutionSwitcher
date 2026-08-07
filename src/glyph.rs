pub const SIDE: u32 = 32;

pub const INK: [u8; 3] = [0x4a, 0x6f, 0x8a];

pub fn lit(x: u32, y: u32) -> bool {
    let screen = (2..30).contains(&x) && (4..24).contains(&y);
    let inside = (5..27).contains(&x) && (7..21).contains(&y);
    let stand = (14..18).contains(&x) && (24..27).contains(&y);
    let base = (9..23).contains(&x) && (27..30).contains(&y);
    (screen && !inside) || stand || base
}
