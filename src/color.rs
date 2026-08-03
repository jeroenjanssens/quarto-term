use avt::Color;

pub fn indexed_color_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0 => (0, 0, 0),
        1 => (205, 49, 49),
        2 => (13, 188, 121),
        3 => (229, 229, 16),
        4 => (36, 114, 200),
        5 => (188, 63, 188),
        6 => (17, 168, 205),
        7 => (229, 229, 229),
        8 => (102, 102, 102),
        9 => (241, 76, 76),
        10 => (35, 209, 139),
        11 => (245, 245, 67),
        12 => (59, 142, 234),
        13 => (214, 112, 214),
        14 => (41, 184, 219),
        15 => (229, 229, 229),
        i if i < 232 => {
            let idx = i - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            let to_byte = |v: u8| -> u8 { if v == 0 { 0 } else { 55 + v * 40 } };
            (to_byte(r), to_byte(g), to_byte(b))
        }
        i => {
            let l = 8 + 10 * (i as u16 - 232) as u8;
            (l, l, l)
        }
    }
}

pub fn color_to_hex(color: Color) -> String {
    match color {
        Color::RGB(rgb) => format!("{:02X}{:02X}{:02X}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => {
            let (r, g, b) = indexed_color_rgb(i);
            format!("{:02X}{:02X}{:02X}", r, g, b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi16_colors() {
        assert_eq!(indexed_color_rgb(0), (0, 0, 0));
        assert_eq!(indexed_color_rgb(1), (205, 49, 49));
        assert_eq!(indexed_color_rgb(15), (229, 229, 229));
    }

    #[test]
    fn cube_colors() {
        assert_eq!(indexed_color_rgb(16), (0, 0, 0));
        // index 196 = 16 + 5*36 + 0*6 + 0 = pure red
        assert_eq!(indexed_color_rgb(196), (255, 0, 0));
        // index 231 = 16 + 5*36 + 5*6 + 5 = white
        assert_eq!(indexed_color_rgb(231), (255, 255, 255));
    }

    #[test]
    fn grayscale_colors() {
        assert_eq!(indexed_color_rgb(232), (8, 8, 8));
        assert_eq!(indexed_color_rgb(255), (238, 238, 238));
    }

    #[test]
    fn color_to_hex_rgb() {
        assert_eq!(color_to_hex(Color::rgb(255, 0, 128)), "FF0080");
    }

    #[test]
    fn color_to_hex_indexed() {
        assert_eq!(color_to_hex(Color::Indexed(1)), "CD3131");
        assert_eq!(color_to_hex(Color::Indexed(232)), "080808");
    }
}
