pub struct KeyTranslation {
    pub bytes: Vec<u8>,
    pub is_named: bool,
}

struct NamedKey {
    aliases: &'static [&'static str],
    bytes: &'static [u8],
    csi_modified: &'static [u8],
}

static NAMED_KEYS: &[NamedKey] = &[
    NamedKey { aliases: &["enter", "return", "cr"], bytes: b"\r", csi_modified: b"\x1b[13u" },
    NamedKey { aliases: &["tab"], bytes: b"\t", csi_modified: b"\x1b[9u" },
    NamedKey { aliases: &["escape", "esc"], bytes: b"\x1b", csi_modified: b"\x1b[27u" },
    NamedKey { aliases: &["backspace", "bs"], bytes: b"\x7f", csi_modified: b"\x1b[127u" },
    NamedKey { aliases: &["delete", "del"], bytes: b"\x1b[3~", csi_modified: b"\x1b[3~" },
    NamedKey { aliases: &["space"], bytes: b" ", csi_modified: b"\x1b[32u" },
    NamedKey { aliases: &["up"], bytes: b"\x1b[A", csi_modified: b"\x1b[A" },
    NamedKey { aliases: &["down"], bytes: b"\x1b[B", csi_modified: b"\x1b[B" },
    NamedKey { aliases: &["right"], bytes: b"\x1b[C", csi_modified: b"\x1b[C" },
    NamedKey { aliases: &["left"], bytes: b"\x1b[D", csi_modified: b"\x1b[D" },
    NamedKey { aliases: &["home"], bytes: b"\x1b[H", csi_modified: b"\x1b[H" },
    NamedKey { aliases: &["end"], bytes: b"\x1b[F", csi_modified: b"\x1b[F" },
    NamedKey { aliases: &["pageup", "page-up", "page_up"], bytes: b"\x1b[5~", csi_modified: b"\x1b[5~" },
    NamedKey { aliases: &["pagedown", "page-down", "page_down"], bytes: b"\x1b[6~", csi_modified: b"\x1b[6~" },
    NamedKey { aliases: &["insert"], bytes: b"\x1b[2~", csi_modified: b"\x1b[2~" },
    NamedKey { aliases: &["f1"], bytes: b"\x1bOP", csi_modified: b"\x1bOP" },
    NamedKey { aliases: &["f2"], bytes: b"\x1bOQ", csi_modified: b"\x1bOQ" },
    NamedKey { aliases: &["f3"], bytes: b"\x1bOR", csi_modified: b"\x1bOR" },
    NamedKey { aliases: &["f4"], bytes: b"\x1bOS", csi_modified: b"\x1bOS" },
    NamedKey { aliases: &["f5"], bytes: b"\x1b[15~", csi_modified: b"\x1b[15~" },
    NamedKey { aliases: &["f6"], bytes: b"\x1b[17~", csi_modified: b"\x1b[17~" },
    NamedKey { aliases: &["f7"], bytes: b"\x1b[18~", csi_modified: b"\x1b[18~" },
    NamedKey { aliases: &["f8"], bytes: b"\x1b[19~", csi_modified: b"\x1b[19~" },
    NamedKey { aliases: &["f9"], bytes: b"\x1b[20~", csi_modified: b"\x1b[20~" },
    NamedKey { aliases: &["f10"], bytes: b"\x1b[21~", csi_modified: b"\x1b[21~" },
    NamedKey { aliases: &["f11"], bytes: b"\x1b[23~", csi_modified: b"\x1b[23~" },
    NamedKey { aliases: &["f12"], bytes: b"\x1b[24~", csi_modified: b"\x1b[24~" },
];

fn find_named_key(name: &str) -> Option<&'static NamedKey> {
    NAMED_KEYS.iter().find(|k| k.aliases.contains(&name))
}

pub fn translate(name: &str) -> KeyTranslation {
    let lower = name.to_lowercase();
    let lower = lower.trim();

    if let Some(bytes) = parse_modified_key(lower) {
        return KeyTranslation { bytes, is_named: true };
    }

    if let Some(key) = find_named_key(lower) {
        return KeyTranslation { bytes: key.bytes.to_vec(), is_named: true };
    }

    KeyTranslation { bytes: name.as_bytes().to_vec(), is_named: false }
}

#[cfg(test)]
pub fn translate_keycode(name: &str) -> Vec<u8> {
    translate(name).bytes
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

    let modifier_param = modifier_param_value(&modifiers);

    if let Some(named) = find_named_key(key) {
        return Some(apply_modifier_to_csi(named.csi_modified, modifier_param));
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

fn apply_modifier_to_csi(seq: &[u8], modifier: u8) -> Vec<u8> {
    let s = String::from_utf8_lossy(seq);

    // CSI u format: ESC [ code u → ESC [ code ; modifier u
    if s.ends_with('u') {
        let inner = &s[2..s.len() - 1];
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
        let inner = &s[2..s.len() - 1];
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
        assert_eq!(translate_keycode("alt-a"), b"\x1b[97;3u".to_vec());
    }

    #[test]
    fn test_ctrl_shift_a() {
        assert_eq!(translate_keycode("ctrl-shift-a"), b"\x1b[97;6u".to_vec());
    }

    #[test]
    fn test_ctrl_alt_a() {
        assert_eq!(translate_keycode("ctrl-alt-a"), b"\x1b[97;7u".to_vec());
    }

    #[test]
    fn test_shift_up() {
        assert_eq!(translate_keycode("shift-up"), b"\x1b[1;2A".to_vec());
    }

    #[test]
    fn test_ctrl_shift_up() {
        assert_eq!(translate_keycode("ctrl-shift-up"), b"\x1b[1;6A".to_vec());
    }

    #[test]
    fn test_alt_f1() {
        assert_eq!(translate_keycode("alt-f1"), b"\x1b[1;3P".to_vec());
    }

    #[test]
    fn test_shift_delete() {
        assert_eq!(translate_keycode("shift-delete"), b"\x1b[3;2~".to_vec());
    }

    #[test]
    fn test_meta_prefix() {
        assert_eq!(translate_keycode("meta-a"), translate_keycode("alt-a"));
        assert_eq!(translate_keycode("m-a"), translate_keycode("alt-a"));
    }

    #[test]
    fn test_ctrl_alt_shift() {
        assert_eq!(translate_keycode("ctrl-alt-shift-a"), b"\x1b[97;8u".to_vec());
    }

    #[test]
    fn is_named_known_keys() {
        assert!(translate("enter").is_named);
        assert!(translate("tab").is_named);
        assert!(translate("escape").is_named);
        assert!(translate("up").is_named);
        assert!(translate("f1").is_named);
        assert!(translate("backspace").is_named);
        assert!(translate("page-up").is_named);
        assert!(translate("page_down").is_named);
    }

    #[test]
    fn is_named_ctrl_prefix() {
        assert!(translate("ctrl-c").is_named);
        assert!(translate("c-x").is_named);
        assert!(translate("shift-up").is_named);
        assert!(translate("alt-f1").is_named);
    }

    #[test]
    fn is_named_plain_text_not_named() {
        assert!(!translate("hello").is_named);
        assert!(!translate("a").is_named);
        assert!(!translate("echo").is_named);
    }

    #[test]
    fn is_named_case_insensitive() {
        assert!(translate("Enter").is_named);
        assert!(translate("TAB").is_named);
        assert!(translate("Ctrl-C").is_named);
    }
}
