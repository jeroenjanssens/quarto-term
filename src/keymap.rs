pub fn translate_keycode(name: &str) -> Vec<u8> {
    let lower = name.to_lowercase();
    let lower = lower.trim();

    if let Some(byte) = parse_ctrl_combo(lower) {
        return vec![byte];
    }

    match lower {
        "enter" | "return" | "cr" => vec![b'\r'],
        "tab" => vec![b'\t'],
        "escape" | "esc" => vec![0x1b],
        "backspace" | "bs" => vec![0x7f],
        "delete" | "del" => b"\x1b[3~".to_vec(),
        "space" => vec![b' '],

        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "pageup" | "page-up" | "page_up" => b"\x1b[5~".to_vec(),
        "pagedown" | "page-down" | "page_down" => b"\x1b[6~".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),

        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        "f5" => b"\x1b[15~".to_vec(),
        "f6" => b"\x1b[17~".to_vec(),
        "f7" => b"\x1b[18~".to_vec(),
        "f8" => b"\x1b[19~".to_vec(),
        "f9" => b"\x1b[20~".to_vec(),
        "f10" => b"\x1b[21~".to_vec(),
        "f11" => b"\x1b[23~".to_vec(),
        "f12" => b"\x1b[24~".to_vec(),

        _ => name.as_bytes().to_vec(),
    }
}

fn parse_ctrl_combo(s: &str) -> Option<u8> {
    let s = s.strip_prefix("ctrl-").or_else(|| s.strip_prefix("c-"))?;
    if s.len() == 1 {
        let ch = s.chars().next()?;
        match ch {
            'a'..='z' => Some(ch as u8 - b'a' + 1),
            '[' => Some(0x1b),
            '\\' => Some(0x1c),
            ']' => Some(0x1d),
            '^' => Some(0x1e),
            '_' => Some(0x1f),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctrl_c() {
        assert_eq!(translate_keycode("ctrl-c"), vec![0x03]);
        assert_eq!(translate_keycode("Ctrl-C"), vec![0x03]);
        assert_eq!(translate_keycode("C-c"), vec![0x03]);
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(translate_keycode("up"), b"\x1b[A".to_vec());
        assert_eq!(translate_keycode("Down"), b"\x1b[B".to_vec());
    }

    #[test]
    fn test_enter() {
        assert_eq!(translate_keycode("enter"), vec![b'\r']);
        assert_eq!(translate_keycode("Enter"), vec![b'\r']);
    }

    #[test]
    fn test_fallback_raw() {
        assert_eq!(translate_keycode("q"), b"q".to_vec());
        assert_eq!(translate_keycode("hello"), b"hello".to_vec());
    }
}
