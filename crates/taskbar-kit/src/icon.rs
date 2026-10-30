//! Generated 32x32 RGBA tray icons: a keycap with a single letter/digit.

/// A keycap with `ch` (5x7 bitmap scaled x3, centered), like keymap-rs's "K".
pub fn letter_rgba(ch: char, fill: [u8; 4], edge: [u8; 4]) -> Vec<u8> {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];
    let put = |px: &mut [u8], x: usize, y: usize, c: [u8; 4]| {
        if x < S && y < S {
            let i = (y * S + x) * 4;
            px[i..i + 4].copy_from_slice(&c);
        }
    };

    let white = [0xff, 0xff, 0xff, 0xff];
    for y in 0..S {
        for x in 0..S {
            let corner = |cx: usize, cy: usize| {
                (x as i32 - cx as i32).pow(2) + (y as i32 - cy as i32).pow(2) <= 16
            };
            let inside = (2..30).contains(&x) && (2..30).contains(&y)
                || corner(6, 6)
                || corner(25, 6)
                || corner(6, 25)
                || corner(25, 25);
            if inside {
                let border = !(4..28).contains(&x) || !(4..28).contains(&y);
                put(&mut px, x, y, if border { edge } else { fill });
            }
        }
    }
    if let Some(glyph) = glyph(ch) {
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits >> (4 - col) & 1 == 1 {
                    for dy in 0..3 {
                        for dx in 0..3 {
                            put(&mut px, 9 + col * 3 + dx, 5 + row * 3 + dy, white);
                        }
                    }
                }
            }
        }
    }
    px
}

fn glyph(ch: char) -> Option<[u8; 7]> {
    Some(match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_32x32_rgba() {
        let px = letter_rgba('t', [0x3b, 0x6e, 0xa5, 0xff], [0x23, 0x46, 0x6e, 0xff]);
        assert_eq!(px.len(), 32 * 32 * 4);
    }
}
