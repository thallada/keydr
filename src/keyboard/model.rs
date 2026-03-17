use std::sync::OnceLock;

use crate::keyboard::display::{BACKSPACE, ENTER, SPACE, TAB};
use crate::keyboard::finger::{Finger, FingerAssignment, Hand};

#[derive(Clone, Copy, Debug)]
pub struct PhysicalKey {
    pub base: char,
    pub shifted: char,
}

#[derive(Clone, Copy, Debug)]
struct ProfileKeySpec {
    physical: PhysicalKey,
    finger: FingerAssignment,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct KeyboardGeometryHints {
    pub row_offsets: &'static [u16],
    pub key_unit_width: u16,
    pub key_unit_gap: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ModifierPlacementMetadata {
    pub tab_hand: Hand,
    pub enter_hand: Hand,
    pub backspace_hand: Hand,
    pub space_hand: Hand,
}

#[derive(Clone, Copy, Debug)]
struct KeyboardProfile {
    key: &'static str,
    rows: &'static [&'static [ProfileKeySpec]],
    geometry_hints: KeyboardGeometryHints,
    modifier_placement: ModifierPlacementMetadata,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct KeyboardModel {
    pub rows: Vec<Vec<PhysicalKey>>,
    finger_rows: Vec<Vec<FingerAssignment>>,
    pub layout_key: &'static str,
    pub geometry_hints: KeyboardGeometryHints,
    pub modifier_placement: ModifierPlacementMetadata,
}

macro_rules! key {
    ($base:expr, $shifted:expr, $hand:ident, $finger:ident) => {
        ProfileKeySpec {
            physical: PhysicalKey {
                base: $base,
                shifted: $shifted,
            },
            finger: FingerAssignment {
                hand: Hand::$hand,
                finger: Finger::$finger,
            },
        }
    };
}

const STAGGERED_GEOMETRY: KeyboardGeometryHints = KeyboardGeometryHints {
    row_offsets: &[0, 2, 3, 4],
    key_unit_width: 4,
    key_unit_gap: 1,
};

const STANDARD_MODIFIERS: ModifierPlacementMetadata = ModifierPlacementMetadata {
    tab_hand: Hand::Left,
    enter_hand: Hand::Right,
    backspace_hand: Hand::Right,
    space_hand: Hand::Right,
};

const QWERTY_ROW0: &[ProfileKeySpec] = &[
    key!('`', '~', Left, Pinky),
    key!('1', '!', Left, Pinky),
    key!('2', '@', Left, Ring),
    key!('3', '#', Left, Middle),
    key!('4', '$', Left, Index),
    key!('5', '%', Left, Index),
    key!('6', '^', Right, Index),
    key!('7', '&', Right, Index),
    key!('8', '*', Right, Middle),
    key!('9', '(', Right, Ring),
    key!('0', ')', Right, Pinky),
    key!('-', '_', Right, Pinky),
    key!('=', '+', Right, Pinky),
];

const QWERTY_ROW1: &[ProfileKeySpec] = &[
    key!('q', 'Q', Left, Pinky),
    key!('w', 'W', Left, Ring),
    key!('e', 'E', Left, Middle),
    key!('r', 'R', Left, Index),
    key!('t', 'T', Left, Index),
    key!('y', 'Y', Right, Index),
    key!('u', 'U', Right, Index),
    key!('i', 'I', Right, Middle),
    key!('o', 'O', Right, Ring),
    key!('p', 'P', Right, Pinky),
    key!('[', '{', Right, Pinky),
    key!(']', '}', Right, Pinky),
    key!('\\', '|', Right, Pinky),
];

const QWERTY_ROW2: &[ProfileKeySpec] = &[
    key!('a', 'A', Left, Pinky),
    key!('s', 'S', Left, Ring),
    key!('d', 'D', Left, Middle),
    key!('f', 'F', Left, Index),
    key!('g', 'G', Left, Index),
    key!('h', 'H', Right, Index),
    key!('j', 'J', Right, Index),
    key!('k', 'K', Right, Middle),
    key!('l', 'L', Right, Ring),
    key!(';', ':', Right, Pinky),
    key!('\'', '"', Right, Pinky),
];

const QWERTY_ROW3: &[ProfileKeySpec] = &[
    key!('z', 'Z', Left, Pinky),
    key!('x', 'X', Left, Ring),
    key!('c', 'C', Left, Middle),
    key!('v', 'V', Left, Index),
    key!('b', 'B', Left, Index),
    key!('n', 'N', Right, Index),
    key!('m', 'M', Right, Index),
    key!(',', '<', Right, Middle),
    key!('.', '>', Right, Ring),
    key!('/', '?', Right, Pinky),
];

const DVORAK_ROW0: &[ProfileKeySpec] = &[
    key!('`', '~', Left, Pinky),
    key!('1', '!', Left, Pinky),
    key!('2', '@', Left, Ring),
    key!('3', '#', Left, Middle),
    key!('4', '$', Left, Index),
    key!('5', '%', Left, Index),
    key!('6', '^', Right, Index),
    key!('7', '&', Right, Index),
    key!('8', '*', Right, Middle),
    key!('9', '(', Right, Ring),
    key!('0', ')', Right, Pinky),
    key!('[', '{', Right, Pinky),
    key!(']', '}', Right, Pinky),
];

const DVORAK_ROW1: &[ProfileKeySpec] = &[
    key!('\'', '"', Left, Pinky),
    key!(',', '<', Left, Ring),
    key!('.', '>', Left, Middle),
    key!('p', 'P', Left, Index),
    key!('y', 'Y', Left, Index),
    key!('f', 'F', Right, Index),
    key!('g', 'G', Right, Index),
    key!('c', 'C', Right, Middle),
    key!('r', 'R', Right, Ring),
    key!('l', 'L', Right, Pinky),
    key!('/', '?', Right, Pinky),
    key!('=', '+', Right, Pinky),
    key!('\\', '|', Right, Pinky),
];

const DVORAK_ROW2: &[ProfileKeySpec] = &[
    key!('a', 'A', Left, Pinky),
    key!('o', 'O', Left, Ring),
    key!('e', 'E', Left, Middle),
    key!('u', 'U', Left, Index),
    key!('i', 'I', Left, Index),
    key!('d', 'D', Right, Index),
    key!('h', 'H', Right, Index),
    key!('t', 'T', Right, Middle),
    key!('n', 'N', Right, Ring),
    key!('s', 'S', Right, Pinky),
    key!('-', '_', Right, Pinky),
];

const DVORAK_ROW3: &[ProfileKeySpec] = &[
    key!(';', ':', Left, Pinky),
    key!('q', 'Q', Left, Ring),
    key!('j', 'J', Left, Middle),
    key!('k', 'K', Left, Index),
    key!('x', 'X', Left, Index),
    key!('b', 'B', Right, Index),
    key!('m', 'M', Right, Index),
    key!('w', 'W', Right, Middle),
    key!('v', 'V', Right, Ring),
    key!('z', 'Z', Right, Pinky),
];

const COLEMAK_ROW0: &[ProfileKeySpec] = QWERTY_ROW0;

const COLEMAK_ROW1: &[ProfileKeySpec] = &[
    key!('q', 'Q', Left, Pinky),
    key!('w', 'W', Left, Ring),
    key!('f', 'F', Left, Middle),
    key!('p', 'P', Left, Index),
    key!('g', 'G', Left, Index),
    key!('j', 'J', Right, Index),
    key!('l', 'L', Right, Index),
    key!('u', 'U', Right, Middle),
    key!('y', 'Y', Right, Ring),
    key!(';', ':', Right, Pinky),
    key!('[', '{', Right, Pinky),
    key!(']', '}', Right, Pinky),
    key!('\\', '|', Right, Pinky),
];

const COLEMAK_ROW2: &[ProfileKeySpec] = &[
    key!('a', 'A', Left, Pinky),
    key!('r', 'R', Left, Ring),
    key!('s', 'S', Left, Middle),
    key!('t', 'T', Left, Index),
    key!('d', 'D', Left, Index),
    key!('h', 'H', Right, Index),
    key!('n', 'N', Right, Index),
    key!('e', 'E', Right, Middle),
    key!('i', 'I', Right, Ring),
    key!('o', 'O', Right, Pinky),
    key!('\'', '"', Right, Pinky),
];

const COLEMAK_ROW3: &[ProfileKeySpec] = &[
    key!('z', 'Z', Left, Pinky),
    key!('x', 'X', Left, Ring),
    key!('c', 'C', Left, Middle),
    key!('v', 'V', Left, Index),
    key!('b', 'B', Left, Index),
    key!('k', 'K', Right, Index),
    key!('m', 'M', Right, Index),
    key!(',', '<', Right, Middle),
    key!('.', '>', Right, Ring),
    key!('/', '?', Right, Pinky),
];

const DE_QWERTZ_ROW0: &[ProfileKeySpec] = &[
    key!('^', '°', Left, Pinky),
    key!('1', '!', Left, Pinky),
    key!('2', '"', Left, Ring),
    key!('3', '§', Left, Middle),
    key!('4', '$', Left, Index),
    key!('5', '%', Left, Index),
    key!('6', '&', Right, Index),
    key!('7', '/', Right, Index),
    key!('8', '(', Right, Middle),
    key!('9', ')', Right, Ring),
    key!('0', '=', Right, Pinky),
    key!('ß', '?', Right, Pinky),
    key!('´', '`', Right, Pinky),
];

const DE_QWERTZ_ROW1: &[ProfileKeySpec] = &[
    key!('q', 'Q', Left, Pinky),
    key!('w', 'W', Left, Ring),
    key!('e', 'E', Left, Middle),
    key!('r', 'R', Left, Index),
    key!('t', 'T', Left, Index),
    key!('z', 'Z', Right, Index),
    key!('u', 'U', Right, Index),
    key!('i', 'I', Right, Middle),
    key!('o', 'O', Right, Ring),
    key!('p', 'P', Right, Pinky),
    key!('ü', 'Ü', Right, Pinky),
    key!('+', '*', Right, Pinky),
    key!('#', '\'', Right, Pinky),
];

const DE_QWERTZ_ROW2: &[ProfileKeySpec] = &[
    key!('a', 'A', Left, Pinky),
    key!('s', 'S', Left, Ring),
    key!('d', 'D', Left, Middle),
    key!('f', 'F', Left, Index),
    key!('g', 'G', Left, Index),
    key!('h', 'H', Right, Index),
    key!('j', 'J', Right, Index),
    key!('k', 'K', Right, Middle),
    key!('l', 'L', Right, Ring),
    key!('ö', 'Ö', Right, Pinky),
    key!('ä', 'Ä', Right, Pinky),
];

const DE_QWERTZ_ROW3: &[ProfileKeySpec] = &[
    key!('y', 'Y', Left, Pinky),
    key!('x', 'X', Left, Ring),
    key!('c', 'C', Left, Middle),
    key!('v', 'V', Left, Index),
    key!('b', 'B', Left, Index),
    key!('n', 'N', Right, Index),
    key!('m', 'M', Right, Index),
    key!(',', ';', Right, Middle),
    key!('.', ':', Right, Ring),
    key!('-', '_', Right, Pinky),
];

const FR_AZERTY_ROW0: &[ProfileKeySpec] = &[
    key!('²', '~', Left, Pinky),
    key!('&', '1', Left, Pinky),
    key!('é', '2', Left, Ring),
    key!('"', '3', Left, Middle),
    key!('\'', '4', Left, Index),
    key!('(', '5', Left, Index),
    key!('-', '6', Right, Index),
    key!('è', '7', Right, Index),
    key!('_', '8', Right, Middle),
    key!('ç', '9', Right, Ring),
    key!('à', '0', Right, Pinky),
    key!(')', '°', Right, Pinky),
    key!('=', '+', Right, Pinky),
];

const FR_AZERTY_ROW1: &[ProfileKeySpec] = &[
    key!('a', 'A', Left, Pinky),
    key!('z', 'Z', Left, Ring),
    key!('e', 'E', Left, Middle),
    key!('r', 'R', Left, Index),
    key!('t', 'T', Left, Index),
    key!('y', 'Y', Right, Index),
    key!('u', 'U', Right, Index),
    key!('i', 'I', Right, Middle),
    key!('o', 'O', Right, Ring),
    key!('p', 'P', Right, Pinky),
    key!('^', '¨', Right, Pinky),
    key!('$', '£', Right, Pinky),
    key!('*', 'µ', Right, Pinky),
];

const FR_AZERTY_ROW2: &[ProfileKeySpec] = &[
    key!('q', 'Q', Left, Pinky),
    key!('s', 'S', Left, Ring),
    key!('d', 'D', Left, Middle),
    key!('f', 'F', Left, Index),
    key!('g', 'G', Left, Index),
    key!('h', 'H', Right, Index),
    key!('j', 'J', Right, Index),
    key!('k', 'K', Right, Middle),
    key!('l', 'L', Right, Ring),
    key!('m', 'M', Right, Pinky),
    key!('ù', '%', Right, Pinky),
];

const FR_AZERTY_ROW3: &[ProfileKeySpec] = &[
    key!('w', 'W', Left, Pinky),
    key!('x', 'X', Left, Ring),
    key!('c', 'C', Left, Middle),
    key!('v', 'V', Left, Index),
    key!('b', 'B', Left, Index),
    key!('n', 'N', Right, Index),
    key!(',', '?', Right, Index),
    key!(';', '.', Right, Middle),
    key!(':', '/', Right, Ring),
    key!('!', '§', Right, Pinky),
];

const QWERTY_ROWS: &[&[ProfileKeySpec]] = &[QWERTY_ROW0, QWERTY_ROW1, QWERTY_ROW2, QWERTY_ROW3];
const DVORAK_ROWS: &[&[ProfileKeySpec]] = &[DVORAK_ROW0, DVORAK_ROW1, DVORAK_ROW2, DVORAK_ROW3];
const COLEMAK_ROWS: &[&[ProfileKeySpec]] =
    &[COLEMAK_ROW0, COLEMAK_ROW1, COLEMAK_ROW2, COLEMAK_ROW3];
const DE_QWERTZ_ROWS: &[&[ProfileKeySpec]] = &[
    DE_QWERTZ_ROW0,
    DE_QWERTZ_ROW1,
    DE_QWERTZ_ROW2,
    DE_QWERTZ_ROW3,
];
const FR_AZERTY_ROWS: &[&[ProfileKeySpec]] = &[
    FR_AZERTY_ROW0,
    FR_AZERTY_ROW1,
    FR_AZERTY_ROW2,
    FR_AZERTY_ROW3,
];

const QWERTY_PROFILE: KeyboardProfile = KeyboardProfile {
    key: "qwerty",
    rows: QWERTY_ROWS,
    geometry_hints: STAGGERED_GEOMETRY,
    modifier_placement: STANDARD_MODIFIERS,
};

const DVORAK_PROFILE: KeyboardProfile = KeyboardProfile {
    key: "dvorak",
    rows: DVORAK_ROWS,
    geometry_hints: STAGGERED_GEOMETRY,
    modifier_placement: STANDARD_MODIFIERS,
};

const COLEMAK_PROFILE: KeyboardProfile = KeyboardProfile {
    key: "colemak",
    rows: COLEMAK_ROWS,
    geometry_hints: STAGGERED_GEOMETRY,
    modifier_placement: STANDARD_MODIFIERS,
};

const DE_QWERTZ_PROFILE: KeyboardProfile = KeyboardProfile {
    key: "de_qwertz",
    rows: DE_QWERTZ_ROWS,
    geometry_hints: STAGGERED_GEOMETRY,
    modifier_placement: STANDARD_MODIFIERS,
};

const FR_AZERTY_PROFILE: KeyboardProfile = KeyboardProfile {
    key: "fr_azerty",
    rows: FR_AZERTY_ROWS,
    geometry_hints: STAGGERED_GEOMETRY,
    modifier_placement: STANDARD_MODIFIERS,
};

const KEYBOARD_PROFILES: &[KeyboardProfile] = &[
    QWERTY_PROFILE,
    DVORAK_PROFILE,
    COLEMAK_PROFILE,
    DE_QWERTZ_PROFILE,
    FR_AZERTY_PROFILE,
];

const EXTRA_LAYOUT_KEYS: &[&str] = &[
    "es_intl", "it_intl", "pt_intl", "nl_intl", "sv_intl", "da_intl", "nb_intl", "fi_intl",
    "pl_intl", "cs_intl", "ro_intl", "hr_intl", "hu_intl", "lt_intl", "lv_intl", "sl_intl",
    "et_intl", "tr_intl",
];

const ES_INTL_CHARS: &[char] = &['ñ', 'á', 'é', 'í', 'ó', 'ú'];
const IT_INTL_CHARS: &[char] = &['à', 'è', 'é', 'ì', 'í', 'î', 'ò', 'ó', 'ù'];
const PT_INTL_CHARS: &[char] = &['ã', 'ç', 'á', 'à', 'â', 'é', 'ê', 'í', 'ó', 'ô', 'õ', 'ú'];
const NL_INTL_CHARS: &[char] = &['ë', 'ï'];
const SV_INTL_CHARS: &[char] = &['å', 'ä', 'ö'];
const DA_INTL_CHARS: &[char] = &['æ', 'ø', 'å'];
const NB_INTL_CHARS: &[char] = &['æ', 'ø', 'å'];
const FI_INTL_CHARS: &[char] = &['ä', 'ö', 'å'];
const PL_INTL_CHARS: &[char] = &['ą', 'ć', 'ę', 'ł', 'ń', 'ó', 'ś', 'ź', 'ż'];
const CS_INTL_CHARS: &[char] = &[
    'á', 'č', 'ď', 'é', 'ě', 'í', 'ň', 'ó', 'ř', 'š', 'ť', 'ú', 'ů', 'ý', 'ž',
];
const RO_INTL_CHARS: &[char] = &['ă', 'â', 'î', 'ș', 'ț'];
const HR_INTL_CHARS: &[char] = &['č', 'ć', 'đ', 'š', 'ž'];
const HU_INTL_CHARS: &[char] = &['á', 'é', 'í', 'ó', 'ö', 'ő', 'ú', 'ü', 'ű'];
const LT_INTL_CHARS: &[char] = &['ą', 'č', 'ę', 'ė', 'į', 'š', 'ų', 'ū', 'ž'];
const LV_INTL_CHARS: &[char] = &['ā', 'č', 'ē', 'ģ', 'ī', 'ķ', 'ļ', 'ņ', 'š', 'ū', 'ž'];
const SL_INTL_CHARS: &[char] = &['č', 'š', 'ž'];
const ET_INTL_CHARS: &[char] = &['ä', 'ö', 'õ', 'ü', 'š', 'ž'];
const TR_INTL_CHARS: &[char] = &['ç', 'ğ', 'ı', 'ö', 'ş', 'ü'];

const LOCALE_OVERLAY_SLOTS: &[(usize, usize)] = &[
    (1, 10),
    (1, 11),
    (1, 12),
    (2, 9),
    (2, 10),
    (3, 7),
    (3, 8),
    (3, 9),
    (0, 11),
    (0, 12),
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (0, 8),
    (0, 9),
    (0, 10),
];

impl KeyboardModel {
    pub fn supported_layout_keys() -> &'static [&'static str] {
        static KEYS: OnceLock<Vec<&'static str>> = OnceLock::new();
        KEYS.get_or_init(|| {
            let mut keys: Vec<&'static str> = KEYBOARD_PROFILES.iter().map(|p| p.key).collect();
            keys.extend(EXTRA_LAYOUT_KEYS.iter().copied());
            keys
        })
        .as_slice()
    }

    #[allow(dead_code)]
    pub fn qwerty() -> Self {
        Self::from_key("qwerty").expect("qwerty profile must be registered")
    }

    #[allow(dead_code)]
    pub fn dvorak() -> Self {
        Self::from_key("dvorak").expect("dvorak profile must be registered")
    }

    #[allow(dead_code)]
    pub fn colemak() -> Self {
        Self::from_key("colemak").expect("colemak profile must be registered")
    }

    fn qwerty_with_locale(layout_key: &'static str, locale_chars: &[char]) -> Self {
        let mut model = Self::qwerty();
        model.layout_key = layout_key;

        let mut used: std::collections::HashSet<char> = model
            .rows
            .iter()
            .flat_map(|row| row.iter().flat_map(|pk| [pk.base, pk.shifted]))
            .collect();

        let mut slots = LOCALE_OVERLAY_SLOTS.iter();
        for &ch in locale_chars {
            if used.contains(&ch) {
                continue;
            }
            let Some(&(row, col)) = slots.next() else {
                break;
            };
            let candidate = ch.to_uppercase().next().unwrap_or(ch);
            let shifted = if candidate != ch && used.contains(&candidate) {
                ch
            } else {
                candidate
            };
            model.rows[row][col] = PhysicalKey { base: ch, shifted };
            used.insert(ch);
            if shifted != ch {
                used.insert(shifted);
            }
        }
        model
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "es_intl" => return Some(Self::qwerty_with_locale("es_intl", ES_INTL_CHARS)),
            "it_intl" => return Some(Self::qwerty_with_locale("it_intl", IT_INTL_CHARS)),
            "pt_intl" => return Some(Self::qwerty_with_locale("pt_intl", PT_INTL_CHARS)),
            "nl_intl" => return Some(Self::qwerty_with_locale("nl_intl", NL_INTL_CHARS)),
            "sv_intl" => return Some(Self::qwerty_with_locale("sv_intl", SV_INTL_CHARS)),
            "da_intl" => return Some(Self::qwerty_with_locale("da_intl", DA_INTL_CHARS)),
            "nb_intl" => return Some(Self::qwerty_with_locale("nb_intl", NB_INTL_CHARS)),
            "fi_intl" => return Some(Self::qwerty_with_locale("fi_intl", FI_INTL_CHARS)),
            "pl_intl" => return Some(Self::qwerty_with_locale("pl_intl", PL_INTL_CHARS)),
            "cs_intl" => return Some(Self::qwerty_with_locale("cs_intl", CS_INTL_CHARS)),
            "ro_intl" => return Some(Self::qwerty_with_locale("ro_intl", RO_INTL_CHARS)),
            "hr_intl" => return Some(Self::qwerty_with_locale("hr_intl", HR_INTL_CHARS)),
            "hu_intl" => return Some(Self::qwerty_with_locale("hu_intl", HU_INTL_CHARS)),
            "lt_intl" => return Some(Self::qwerty_with_locale("lt_intl", LT_INTL_CHARS)),
            "lv_intl" => return Some(Self::qwerty_with_locale("lv_intl", LV_INTL_CHARS)),
            "sl_intl" => return Some(Self::qwerty_with_locale("sl_intl", SL_INTL_CHARS)),
            "et_intl" => return Some(Self::qwerty_with_locale("et_intl", ET_INTL_CHARS)),
            "tr_intl" => return Some(Self::qwerty_with_locale("tr_intl", TR_INTL_CHARS)),
            _ => {}
        }
        let profile = KEYBOARD_PROFILES.iter().find(|p| p.key == key)?;
        let rows = profile
            .rows
            .iter()
            .map(|row| row.iter().map(|spec| spec.physical).collect())
            .collect();
        let finger_rows = profile
            .rows
            .iter()
            .map(|row| row.iter().map(|spec| spec.finger).collect())
            .collect();

        Some(Self {
            rows,
            finger_rows,
            layout_key: profile.key,
            geometry_hints: profile.geometry_hints,
            modifier_placement: profile.modifier_placement,
        })
    }

    /// Given a base character, return its shifted counterpart.
    #[allow(dead_code)]
    pub fn base_to_shifted(&self, ch: char) -> Option<char> {
        self.physical_key_for(ch)
            .filter(|pk| pk.base == ch)
            .map(|pk| pk.shifted)
    }

    /// Given a shifted character, return its base counterpart.
    #[allow(dead_code)]
    pub fn shifted_to_base(&self, ch: char) -> Option<char> {
        self.physical_key_for(ch)
            .filter(|pk| pk.shifted == ch)
            .map(|pk| pk.base)
    }

    pub fn physical_key_for(&self, ch: char) -> Option<&PhysicalKey> {
        self.find_key_position(ch).map(|(r, c)| &self.rows[r][c])
    }

    fn find_key_position(&self, ch: char) -> Option<(usize, usize)> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (col_idx, key) in row.iter().enumerate() {
                if key.base == ch || key.shifted == ch {
                    return Some((row_idx, col_idx));
                }
            }
        }
        None
    }

    /// Get the finger assignment for a physical key by its row/col position.
    pub fn finger_for_position(&self, row: usize, col: usize) -> FingerAssignment {
        self.finger_rows
            .get(row)
            .and_then(|r| r.get(col).copied())
            .unwrap_or(FingerAssignment::new(Hand::Right, Finger::Index))
    }

    /// Get finger assignment for a character, looking it up in the model.
    pub fn finger_for_char(&self, ch: char) -> FingerAssignment {
        match ch {
            TAB => FingerAssignment::new(self.modifier_placement.tab_hand, Finger::Pinky),
            ENTER => FingerAssignment::new(self.modifier_placement.enter_hand, Finger::Pinky),
            BACKSPACE => {
                FingerAssignment::new(self.modifier_placement.backspace_hand, Finger::Pinky)
            }
            SPACE => FingerAssignment::new(self.modifier_placement.space_hand, Finger::Thumb),
            _ => {
                if let Some((row_idx, col_idx)) = self.find_key_position(ch) {
                    self.finger_for_position(row_idx, col_idx)
                } else {
                    FingerAssignment::new(Hand::Right, Finger::Index)
                }
            }
        }
    }

    /// Letter-only rows (rows 1-3) for compact keyboard display.
    pub fn letter_rows(&self) -> &[Vec<PhysicalKey>] {
        if self.rows.len() > 1 {
            &self.rows[1..]
        } else {
            &self.rows
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn canonical_profile_keys_are_registered() {
        assert_eq!(
            KeyboardModel::supported_layout_keys(),
            &[
                "qwerty",
                "dvorak",
                "colemak",
                "de_qwertz",
                "fr_azerty",
                "es_intl",
                "it_intl",
                "pt_intl",
                "nl_intl",
                "sv_intl",
                "da_intl",
                "nb_intl",
                "fi_intl",
                "pl_intl",
                "cs_intl",
                "ro_intl",
                "hr_intl",
                "hu_intl",
                "lt_intl",
                "lv_intl",
                "sl_intl",
                "et_intl",
                "tr_intl",
            ]
        );

        for key in KeyboardModel::supported_layout_keys() {
            assert!(
                KeyboardModel::from_key(key).is_some(),
                "missing keyboard profile for key: {key}"
            );
        }
    }

    #[test]
    fn language_relevant_profiles_include_expected_locale_keys() {
        let de = KeyboardModel::from_key("de_qwertz").expect("de_qwertz must be registered");
        for ch in ['ä', 'ö', 'ü', 'ß'] {
            assert!(
                de.physical_key_for(ch).is_some(),
                "de_qwertz missing locale char {ch}"
            );
        }

        let fr = KeyboardModel::from_key("fr_azerty").expect("fr_azerty must be registered");
        for ch in ['é', 'è', 'ç', 'à'] {
            assert!(
                fr.physical_key_for(ch).is_some(),
                "fr_azerty missing locale char {ch}"
            );
        }

        // AZERTY keeps digits on shifted number-row keys.
        assert_eq!(fr.base_to_shifted('é'), Some('2'));
        assert_eq!(fr.base_to_shifted('à'), Some('0'));
        assert_eq!(fr.shifted_to_base('2'), Some('é'));
        assert_eq!(fr.shifted_to_base('0'), Some('à'));

        let tr = KeyboardModel::from_key("tr_intl").expect("tr_intl must be registered");
        for ch in ['ç', 'ğ', 'ı', 'ö', 'ş', 'ü'] {
            assert!(
                tr.physical_key_for(ch).is_some(),
                "tr_intl missing locale char {ch}"
            );
        }
    }

    #[test]
    fn profile_rows_have_finger_assignment_coverage() {
        for key in KeyboardModel::supported_layout_keys() {
            let model = KeyboardModel::from_key(key).expect("profile should exist");
            assert_eq!(model.rows.len(), model.finger_rows.len());
            for row_idx in 0..model.rows.len() {
                assert_eq!(
                    model.rows[row_idx].len(),
                    model.finger_rows[row_idx].len(),
                    "row {row_idx} mismatch for profile {key}"
                );
            }
        }
    }

    #[test]
    fn profile_character_mapping_is_unique() {
        for key in KeyboardModel::supported_layout_keys() {
            let model = KeyboardModel::from_key(key).expect("profile should exist");
            let mut seen = HashSet::new();
            for row in &model.rows {
                for pk in row {
                    assert!(
                        seen.insert(pk.base),
                        "duplicate base char {:?} in profile {}",
                        pk.base,
                        key
                    );
                    if pk.shifted != pk.base {
                        assert!(
                            seen.insert(pk.shifted),
                            "duplicate shifted char {:?} in profile {}",
                            pk.shifted,
                            key
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_qwerty_covers_all_skill_tree_chars() {
        let model = KeyboardModel::qwerty();

        // All chars used in skill tree branches
        let skill_tree_chars: Vec<char> = vec![
            // Lowercase
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
            'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', // Capitals
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q',
            'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', // Numbers
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', // Prose punctuation
            '.', ',', '\'', ';', ':', '"', '-', '?', '!', '(', ')', // Code symbols
            '=', '+', '*', '/', '{', '}', '[', ']', '<', '>', '&', '|', '^', '~', '@', '#', '$',
            '%', '_', '\\', '`',
        ];

        for ch in &skill_tree_chars {
            assert!(
                model.physical_key_for(*ch).is_some(),
                "KeyboardModel::qwerty() missing char: {:?}",
                ch
            );
        }
    }

    #[test]
    fn test_base_to_shifted_and_back() {
        let model = KeyboardModel::qwerty();

        assert_eq!(model.base_to_shifted('a'), Some('A'));
        assert_eq!(model.base_to_shifted('1'), Some('!'));
        assert_eq!(model.base_to_shifted('['), Some('{'));
        assert_eq!(model.shifted_to_base('A'), Some('a'));
        assert_eq!(model.shifted_to_base('!'), Some('1'));
        assert_eq!(model.shifted_to_base('{'), Some('['));

        // base_to_shifted on a shifted char returns None
        assert_eq!(model.base_to_shifted('A'), None);
        // shifted_to_base on a base char returns None
        assert_eq!(model.shifted_to_base('a'), None);
    }

    #[test]
    fn test_qwerty_has_four_rows() {
        let model = KeyboardModel::qwerty();
        assert_eq!(model.rows.len(), 4);
        assert_eq!(model.rows[0].len(), 13); // number row
        assert_eq!(model.rows[1].len(), 13); // top row
        assert_eq!(model.rows[2].len(), 11); // home row
        assert_eq!(model.rows[3].len(), 10); // bottom row
    }

    #[test]
    fn test_finger_for_char_works_for_all_chars() {
        let model = KeyboardModel::qwerty();
        // Just verify it doesn't panic for various chars
        let _ = model.finger_for_char('a');
        let _ = model.finger_for_char('A');
        let _ = model.finger_for_char('1');
        let _ = model.finger_for_char('!');
        let _ = model.finger_for_char('{');
    }

    #[test]
    fn test_finger_for_meta_keys() {
        let model = KeyboardModel::qwerty();
        assert_eq!(
            model.finger_for_char(TAB),
            FingerAssignment::new(Hand::Left, Finger::Pinky)
        );
        assert_eq!(
            model.finger_for_char(ENTER),
            FingerAssignment::new(Hand::Right, Finger::Pinky)
        );
        assert_eq!(
            model.finger_for_char(BACKSPACE),
            FingerAssignment::new(Hand::Right, Finger::Pinky)
        );
        assert_eq!(
            model.finger_for_char(SPACE),
            FingerAssignment::new(Hand::Right, Finger::Thumb)
        );
    }
}
