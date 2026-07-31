pub fn translate_keycode(name: &str) -> Vec<u8> {
    let lower = name.to_lowercase();
    let lower = lower.trim();

    if let Some(bytes) = parse_modified_key(lower) {
        return bytes;
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

fn parse_modified_key(s: &str) -> Option<Vec<u8>> {
    let (modifiers, key) = parse_modifiers(s)?;

    if modifiers.ctrl && !modifiers.shift && !modifiers.alt {
        if key.len() == 1 {
            let ch = key.chars().next()?;
            return match ch {
                'a'..='z' => Some(vec![ch as u8 - b'a' + 1]),
                '[' => Some(vec![0x1b]),
                '\\' => Some(vec![0x1c]),
                ']' => Some(vec![0x1d]),
                '^' => Some(vec![0x1e]),
                '_' => Some(vec![0x1f]),
                _ => None,
            };
        }
    }

    if modifiers.shift && !modifiers.ctrl && !modifiers.alt {
        if key.len() == 1 {
            let ch = key.chars().next()?;
            if ch.is_ascii_alphabetic() {
                return Some(vec![ch.to_ascii_uppercase() as u8]);
            }
        }
    }

    // CSI u encoding for modifier+key combos (xterm modifyOtherKeys / kitty)
    // Format: ESC [ <keycode> ; <modifier> u
    // Modifier bits: shift=1, alt=2, ctrl=4 → parameter = bits + 1
    let modifier_param = modifier_param_value(&modifiers);

    if let Some(csi_seq) = arrow_or_special_csi(key) {
        return Some(apply_modifier_to_csi(&csi_seq, modifier_param));
    }

    if key.len() == 1 {
        let ch = key.chars().next()?;
        let code = ch as u32;
        return Some(format!("\x1b[{};{}u", code, modifier_param).into_bytes());
    }

    None
}

fn parse_modifiers(s: &str) -> Option<(Modifiers, &str)> {
    let mut mods = Modifiers { ctrl: false, shift: false, alt: false };
    let mut rest = s;
    let mut found_any = false;

    loop {
        if let Some(r) = rest.strip_prefix("ctrl-") {
            mods.ctrl = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("c-") {
            mods.ctrl = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("shift-") {
            mods.shift = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("s-") {
            mods.shift = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("alt-") {
            mods.alt = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("meta-") {
            mods.alt = true;
            rest = r;
            found_any = true;
        } else if let Some(r) = rest.strip_prefix("m-") {
            mods.alt = true;
            rest = r;
            found_any = true;
        } else {
            break;
        }
    }

    if found_any && !rest.is_empty() {
        Some((mods, rest))
    } else {
        None
    }
}

struct Modifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

fn modifier_param_value(mods: &Modifiers) -> u8 {
    let mut bits: u8 = 0;
    if mods.shift { bits |= 1; }
    if mods.alt { bits |= 2; }
    if mods.ctrl { bits |= 4; }
    bits + 1
}

fn arrow_or_special_csi(key: &str) -> Option<Vec<u8>> {
    match key {
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "insert" => Some(b"\x1b[2~".to_vec()),
        "delete" | "del" => Some(b"\x1b[3~".to_vec()),
        "pageup" | "page-up" | "page_up" => Some(b"\x1b[5~".to_vec()),
        "pagedown" | "page-down" | "page_down" => Some(b"\x1b[6~".to_vec()),
        "f1" => Some(b"\x1bOP".to_vec()),
        "f2" => Some(b"\x1bOQ".to_vec()),
        "f3" => Some(b"\x1bOR".to_vec()),
        "f4" => Some(b"\x1bOS".to_vec()),
        "f5" => Some(b"\x1b[15~".to_vec()),
        "f6" => Some(b"\x1b[17~".to_vec()),
        "f7" => Some(b"\x1b[18~".to_vec()),
        "f8" => Some(b"\x1b[19~".to_vec()),
        "f9" => Some(b"\x1b[20~".to_vec()),
        "f10" => Some(b"\x1b[21~".to_vec()),
        "f11" => Some(b"\x1b[23~".to_vec()),
        "f12" => Some(b"\x1b[24~".to_vec()),
        "enter" | "return" | "cr" => Some(b"\x1b[13u".to_vec()),
        "tab" => Some(b"\x1b[9u".to_vec()),
        "escape" | "esc" => Some(b"\x1b[27u".to_vec()),
        "backspace" | "bs" => Some(b"\x1b[127u".to_vec()),
        "space" => Some(b"\x1b[32u".to_vec()),
        _ => None,
    }
}

fn apply_modifier_to_csi(seq: &[u8], modifier: u8) -> Vec<u8> {
    let s = String::from_utf8_lossy(seq);

    // CSI u format: ESC [ code u → ESC [ code ; modifier u
    if s.ends_with('u') {
        let inner = &s[2..s.len() - 1]; // strip ESC[ and u
        return format!("\x1b[{};{}u", inner, modifier).into_bytes();
    }

    // SS3 sequences (F1-F4): ESC O <letter> → ESC [ 1 ; modifier <letter>
    if s.len() == 3 && s.as_bytes()[1] == b'O' {
        let letter = s.as_bytes()[2] as char;
        return format!("\x1b[1;{}{}", modifier, letter).into_bytes();
    }

    // CSI letter sequences (arrows, home, end): ESC [ <letter> → ESC [ 1 ; modifier <letter>
    if s.ends_with(|c: char| c.is_ascii_uppercase()) && !s.contains('~') && !s.contains(';') {
        let letter = s.as_bytes()[s.len() - 1] as char;
        return format!("\x1b[1;{}{}", modifier, letter).into_bytes();
    }

    // CSI number ~ sequences: ESC [ N ~ → ESC [ N ; modifier ~
    if s.ends_with('~') {
        let inner = &s[2..s.len() - 1]; // strip ESC[ and ~
        return format!("\x1b[{};{}~", inner, modifier).into_bytes();
    }

    seq.to_vec()
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

    #[test]
    fn test_shift_a() {
        assert_eq!(translate_keycode("shift-a"), b"A".to_vec());
        assert_eq!(translate_keycode("Shift-Z"), b"Z".to_vec());
    }

    #[test]
    fn test_alt_a() {
        // alt-a → CSI 97;3u (modifier param: alt=2, +1=3)
        assert_eq!(translate_keycode("alt-a"), b"\x1b[97;3u".to_vec());
    }

    #[test]
    fn test_ctrl_shift_a() {
        // ctrl-shift-a → CSI 97;6u (modifier param: shift=1|ctrl=4, +1=6)
        assert_eq!(translate_keycode("ctrl-shift-a"), b"\x1b[97;6u".to_vec());
    }

    #[test]
    fn test_ctrl_alt_a() {
        // ctrl-alt-a → CSI 97;7u (modifier param: alt=2|ctrl=4, +1=7)
        assert_eq!(translate_keycode("ctrl-alt-a"), b"\x1b[97;7u".to_vec());
    }

    #[test]
    fn test_shift_up() {
        // shift-up → ESC[1;2A
        assert_eq!(translate_keycode("shift-up"), b"\x1b[1;2A".to_vec());
    }

    #[test]
    fn test_ctrl_shift_up() {
        // ctrl-shift-up → ESC[1;6A
        assert_eq!(translate_keycode("ctrl-shift-up"), b"\x1b[1;6A".to_vec());
    }

    #[test]
    fn test_alt_f1() {
        // alt-f1 → ESC[1;3P (SS3 with modifier)
        assert_eq!(translate_keycode("alt-f1"), b"\x1b[1;3P".to_vec());
    }

    #[test]
    fn test_shift_delete() {
        // shift-delete → ESC[3;2~
        assert_eq!(translate_keycode("shift-delete"), b"\x1b[3;2~".to_vec());
    }

    #[test]
    fn test_meta_prefix() {
        assert_eq!(translate_keycode("meta-a"), translate_keycode("alt-a"));
        assert_eq!(translate_keycode("m-a"), translate_keycode("alt-a"));
    }

    #[test]
    fn test_ctrl_alt_shift() {
        // ctrl-alt-shift-a → CSI 97;8u (all modifiers: 1|2|4 + 1 = 8)
        assert_eq!(translate_keycode("ctrl-alt-shift-a"), b"\x1b[97;8u".to_vec());
    }
}
