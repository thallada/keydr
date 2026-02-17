use crate::keyboard::finger::{Finger, FingerAssignment, Hand};

#[derive(Clone, Debug)]
pub struct PhysicalKey {
    pub base: char,
    pub shifted: char,
}

#[derive(Clone, Debug)]
pub struct KeyboardModel {
    pub rows: Vec<Vec<PhysicalKey>>,
}

impl KeyboardModel {
    pub fn qwerty() -> Self {
        Self {
            rows: vec![
                vec![
                    PhysicalKey {
                        base: '`',
                        shifted: '~',
                    },
                    PhysicalKey {
                        base: '1',
                        shifted: '!',
                    },
                    PhysicalKey {
                        base: '2',
                        shifted: '@',
                    },
                    PhysicalKey {
                        base: '3',
                        shifted: '#',
                    },
                    PhysicalKey {
                        base: '4',
                        shifted: '$',
                    },
                    PhysicalKey {
                        base: '5',
                        shifted: '%',
                    },
                    PhysicalKey {
                        base: '6',
                        shifted: '^',
                    },
                    PhysicalKey {
                        base: '7',
                        shifted: '&',
                    },
                    PhysicalKey {
                        base: '8',
                        shifted: '*',
                    },
                    PhysicalKey {
                        base: '9',
                        shifted: '(',
                    },
                    PhysicalKey {
                        base: '0',
                        shifted: ')',
                    },
                    PhysicalKey {
                        base: '-',
                        shifted: '_',
                    },
                    PhysicalKey {
                        base: '=',
                        shifted: '+',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'q',
                        shifted: 'Q',
                    },
                    PhysicalKey {
                        base: 'w',
                        shifted: 'W',
                    },
                    PhysicalKey {
                        base: 'e',
                        shifted: 'E',
                    },
                    PhysicalKey {
                        base: 'r',
                        shifted: 'R',
                    },
                    PhysicalKey {
                        base: 't',
                        shifted: 'T',
                    },
                    PhysicalKey {
                        base: 'y',
                        shifted: 'Y',
                    },
                    PhysicalKey {
                        base: 'u',
                        shifted: 'U',
                    },
                    PhysicalKey {
                        base: 'i',
                        shifted: 'I',
                    },
                    PhysicalKey {
                        base: 'o',
                        shifted: 'O',
                    },
                    PhysicalKey {
                        base: 'p',
                        shifted: 'P',
                    },
                    PhysicalKey {
                        base: '[',
                        shifted: '{',
                    },
                    PhysicalKey {
                        base: ']',
                        shifted: '}',
                    },
                    PhysicalKey {
                        base: '\\',
                        shifted: '|',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'a',
                        shifted: 'A',
                    },
                    PhysicalKey {
                        base: 's',
                        shifted: 'S',
                    },
                    PhysicalKey {
                        base: 'd',
                        shifted: 'D',
                    },
                    PhysicalKey {
                        base: 'f',
                        shifted: 'F',
                    },
                    PhysicalKey {
                        base: 'g',
                        shifted: 'G',
                    },
                    PhysicalKey {
                        base: 'h',
                        shifted: 'H',
                    },
                    PhysicalKey {
                        base: 'j',
                        shifted: 'J',
                    },
                    PhysicalKey {
                        base: 'k',
                        shifted: 'K',
                    },
                    PhysicalKey {
                        base: 'l',
                        shifted: 'L',
                    },
                    PhysicalKey {
                        base: ';',
                        shifted: ':',
                    },
                    PhysicalKey {
                        base: '\'',
                        shifted: '"',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'z',
                        shifted: 'Z',
                    },
                    PhysicalKey {
                        base: 'x',
                        shifted: 'X',
                    },
                    PhysicalKey {
                        base: 'c',
                        shifted: 'C',
                    },
                    PhysicalKey {
                        base: 'v',
                        shifted: 'V',
                    },
                    PhysicalKey {
                        base: 'b',
                        shifted: 'B',
                    },
                    PhysicalKey {
                        base: 'n',
                        shifted: 'N',
                    },
                    PhysicalKey {
                        base: 'm',
                        shifted: 'M',
                    },
                    PhysicalKey {
                        base: ',',
                        shifted: '<',
                    },
                    PhysicalKey {
                        base: '.',
                        shifted: '>',
                    },
                    PhysicalKey {
                        base: '/',
                        shifted: '?',
                    },
                ],
            ],
        }
    }

    pub fn dvorak() -> Self {
        Self {
            rows: vec![
                vec![
                    PhysicalKey {
                        base: '`',
                        shifted: '~',
                    },
                    PhysicalKey {
                        base: '1',
                        shifted: '!',
                    },
                    PhysicalKey {
                        base: '2',
                        shifted: '@',
                    },
                    PhysicalKey {
                        base: '3',
                        shifted: '#',
                    },
                    PhysicalKey {
                        base: '4',
                        shifted: '$',
                    },
                    PhysicalKey {
                        base: '5',
                        shifted: '%',
                    },
                    PhysicalKey {
                        base: '6',
                        shifted: '^',
                    },
                    PhysicalKey {
                        base: '7',
                        shifted: '&',
                    },
                    PhysicalKey {
                        base: '8',
                        shifted: '*',
                    },
                    PhysicalKey {
                        base: '9',
                        shifted: '(',
                    },
                    PhysicalKey {
                        base: '0',
                        shifted: ')',
                    },
                    PhysicalKey {
                        base: '[',
                        shifted: '{',
                    },
                    PhysicalKey {
                        base: ']',
                        shifted: '}',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: '\'',
                        shifted: '"',
                    },
                    PhysicalKey {
                        base: ',',
                        shifted: '<',
                    },
                    PhysicalKey {
                        base: '.',
                        shifted: '>',
                    },
                    PhysicalKey {
                        base: 'p',
                        shifted: 'P',
                    },
                    PhysicalKey {
                        base: 'y',
                        shifted: 'Y',
                    },
                    PhysicalKey {
                        base: 'f',
                        shifted: 'F',
                    },
                    PhysicalKey {
                        base: 'g',
                        shifted: 'G',
                    },
                    PhysicalKey {
                        base: 'c',
                        shifted: 'C',
                    },
                    PhysicalKey {
                        base: 'r',
                        shifted: 'R',
                    },
                    PhysicalKey {
                        base: 'l',
                        shifted: 'L',
                    },
                    PhysicalKey {
                        base: '/',
                        shifted: '?',
                    },
                    PhysicalKey {
                        base: '=',
                        shifted: '+',
                    },
                    PhysicalKey {
                        base: '\\',
                        shifted: '|',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'a',
                        shifted: 'A',
                    },
                    PhysicalKey {
                        base: 'o',
                        shifted: 'O',
                    },
                    PhysicalKey {
                        base: 'e',
                        shifted: 'E',
                    },
                    PhysicalKey {
                        base: 'u',
                        shifted: 'U',
                    },
                    PhysicalKey {
                        base: 'i',
                        shifted: 'I',
                    },
                    PhysicalKey {
                        base: 'd',
                        shifted: 'D',
                    },
                    PhysicalKey {
                        base: 'h',
                        shifted: 'H',
                    },
                    PhysicalKey {
                        base: 't',
                        shifted: 'T',
                    },
                    PhysicalKey {
                        base: 'n',
                        shifted: 'N',
                    },
                    PhysicalKey {
                        base: 's',
                        shifted: 'S',
                    },
                    PhysicalKey {
                        base: '-',
                        shifted: '_',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: ';',
                        shifted: ':',
                    },
                    PhysicalKey {
                        base: 'q',
                        shifted: 'Q',
                    },
                    PhysicalKey {
                        base: 'j',
                        shifted: 'J',
                    },
                    PhysicalKey {
                        base: 'k',
                        shifted: 'K',
                    },
                    PhysicalKey {
                        base: 'x',
                        shifted: 'X',
                    },
                    PhysicalKey {
                        base: 'b',
                        shifted: 'B',
                    },
                    PhysicalKey {
                        base: 'm',
                        shifted: 'M',
                    },
                    PhysicalKey {
                        base: 'w',
                        shifted: 'W',
                    },
                    PhysicalKey {
                        base: 'v',
                        shifted: 'V',
                    },
                    PhysicalKey {
                        base: 'z',
                        shifted: 'Z',
                    },
                ],
            ],
        }
    }

    pub fn colemak() -> Self {
        Self {
            rows: vec![
                vec![
                    PhysicalKey {
                        base: '`',
                        shifted: '~',
                    },
                    PhysicalKey {
                        base: '1',
                        shifted: '!',
                    },
                    PhysicalKey {
                        base: '2',
                        shifted: '@',
                    },
                    PhysicalKey {
                        base: '3',
                        shifted: '#',
                    },
                    PhysicalKey {
                        base: '4',
                        shifted: '$',
                    },
                    PhysicalKey {
                        base: '5',
                        shifted: '%',
                    },
                    PhysicalKey {
                        base: '6',
                        shifted: '^',
                    },
                    PhysicalKey {
                        base: '7',
                        shifted: '&',
                    },
                    PhysicalKey {
                        base: '8',
                        shifted: '*',
                    },
                    PhysicalKey {
                        base: '9',
                        shifted: '(',
                    },
                    PhysicalKey {
                        base: '0',
                        shifted: ')',
                    },
                    PhysicalKey {
                        base: '-',
                        shifted: '_',
                    },
                    PhysicalKey {
                        base: '=',
                        shifted: '+',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'q',
                        shifted: 'Q',
                    },
                    PhysicalKey {
                        base: 'w',
                        shifted: 'W',
                    },
                    PhysicalKey {
                        base: 'f',
                        shifted: 'F',
                    },
                    PhysicalKey {
                        base: 'p',
                        shifted: 'P',
                    },
                    PhysicalKey {
                        base: 'g',
                        shifted: 'G',
                    },
                    PhysicalKey {
                        base: 'j',
                        shifted: 'J',
                    },
                    PhysicalKey {
                        base: 'l',
                        shifted: 'L',
                    },
                    PhysicalKey {
                        base: 'u',
                        shifted: 'U',
                    },
                    PhysicalKey {
                        base: 'y',
                        shifted: 'Y',
                    },
                    PhysicalKey {
                        base: ';',
                        shifted: ':',
                    },
                    PhysicalKey {
                        base: '[',
                        shifted: '{',
                    },
                    PhysicalKey {
                        base: ']',
                        shifted: '}',
                    },
                    PhysicalKey {
                        base: '\\',
                        shifted: '|',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'a',
                        shifted: 'A',
                    },
                    PhysicalKey {
                        base: 'r',
                        shifted: 'R',
                    },
                    PhysicalKey {
                        base: 's',
                        shifted: 'S',
                    },
                    PhysicalKey {
                        base: 't',
                        shifted: 'T',
                    },
                    PhysicalKey {
                        base: 'd',
                        shifted: 'D',
                    },
                    PhysicalKey {
                        base: 'h',
                        shifted: 'H',
                    },
                    PhysicalKey {
                        base: 'n',
                        shifted: 'N',
                    },
                    PhysicalKey {
                        base: 'e',
                        shifted: 'E',
                    },
                    PhysicalKey {
                        base: 'i',
                        shifted: 'I',
                    },
                    PhysicalKey {
                        base: 'o',
                        shifted: 'O',
                    },
                    PhysicalKey {
                        base: '\'',
                        shifted: '"',
                    },
                ],
                vec![
                    PhysicalKey {
                        base: 'z',
                        shifted: 'Z',
                    },
                    PhysicalKey {
                        base: 'x',
                        shifted: 'X',
                    },
                    PhysicalKey {
                        base: 'c',
                        shifted: 'C',
                    },
                    PhysicalKey {
                        base: 'v',
                        shifted: 'V',
                    },
                    PhysicalKey {
                        base: 'b',
                        shifted: 'B',
                    },
                    PhysicalKey {
                        base: 'k',
                        shifted: 'K',
                    },
                    PhysicalKey {
                        base: 'm',
                        shifted: 'M',
                    },
                    PhysicalKey {
                        base: ',',
                        shifted: '<',
                    },
                    PhysicalKey {
                        base: '.',
                        shifted: '>',
                    },
                    PhysicalKey {
                        base: '/',
                        shifted: '?',
                    },
                ],
            ],
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "dvorak" => Self::dvorak(),
            "colemak" => Self::colemak(),
            _ => Self::qwerty(),
        }
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
    /// Uses QWERTY-style finger assignments based on column position.
    pub fn finger_for_position(&self, row: usize, col: usize) -> FingerAssignment {
        // Map column to finger based on standard touch-typing
        // Row 0 (number row) has 13 keys, rows 1-3 have varying counts
        // We use column position relative to the keyboard
        let total_cols = self.rows[row].len();

        // For the number row and top row (13 keys each in QWERTY)
        // left pinky: cols 0-1, left ring: col 2, left middle: col 3,
        // left index: cols 4-5, right index: cols 6-7,
        // right middle: col 8, right ring: col 9, right pinky: cols 10+
        match row {
            0 => {
                // Number row
                match col {
                    0 | 1 => FingerAssignment::new(Hand::Left, Finger::Pinky),
                    2 => FingerAssignment::new(Hand::Left, Finger::Ring),
                    3 => FingerAssignment::new(Hand::Left, Finger::Middle),
                    4 | 5 => FingerAssignment::new(Hand::Left, Finger::Index),
                    6 | 7 => FingerAssignment::new(Hand::Right, Finger::Index),
                    8 => FingerAssignment::new(Hand::Right, Finger::Middle),
                    9 => FingerAssignment::new(Hand::Right, Finger::Ring),
                    _ => FingerAssignment::new(Hand::Right, Finger::Pinky),
                }
            }
            1 => {
                // Top row (q-row in QWERTY)
                match col {
                    0 => FingerAssignment::new(Hand::Left, Finger::Pinky),
                    1 => FingerAssignment::new(Hand::Left, Finger::Ring),
                    2 => FingerAssignment::new(Hand::Left, Finger::Middle),
                    3 | 4 => FingerAssignment::new(Hand::Left, Finger::Index),
                    5 | 6 => FingerAssignment::new(Hand::Right, Finger::Index),
                    7 => FingerAssignment::new(Hand::Right, Finger::Middle),
                    8 => FingerAssignment::new(Hand::Right, Finger::Ring),
                    _ => FingerAssignment::new(Hand::Right, Finger::Pinky),
                }
            }
            2 => {
                // Home row
                match col {
                    0 => FingerAssignment::new(Hand::Left, Finger::Pinky),
                    1 => FingerAssignment::new(Hand::Left, Finger::Ring),
                    2 => FingerAssignment::new(Hand::Left, Finger::Middle),
                    3 | 4 => FingerAssignment::new(Hand::Left, Finger::Index),
                    5 | 6 => FingerAssignment::new(Hand::Right, Finger::Index),
                    7 => FingerAssignment::new(Hand::Right, Finger::Middle),
                    8 => FingerAssignment::new(Hand::Right, Finger::Ring),
                    _ => FingerAssignment::new(Hand::Right, Finger::Pinky),
                }
            }
            3 => {
                // Bottom row
                let _ = total_cols;
                match col {
                    0 => FingerAssignment::new(Hand::Left, Finger::Pinky),
                    1 => FingerAssignment::new(Hand::Left, Finger::Ring),
                    2 => FingerAssignment::new(Hand::Left, Finger::Middle),
                    3 | 4 => FingerAssignment::new(Hand::Left, Finger::Index),
                    5 | 6 => FingerAssignment::new(Hand::Right, Finger::Index),
                    7 => FingerAssignment::new(Hand::Right, Finger::Middle),
                    8 => FingerAssignment::new(Hand::Right, Finger::Ring),
                    _ => FingerAssignment::new(Hand::Right, Finger::Pinky),
                }
            }
            _ => FingerAssignment::new(Hand::Right, Finger::Index),
        }
    }

    /// Get finger assignment for a character, looking it up in the model.
    pub fn finger_for_char(&self, ch: char) -> FingerAssignment {
        if let Some((row_idx, col_idx)) = self.find_key_position(ch) {
            self.finger_for_position(row_idx, col_idx)
        } else {
            FingerAssignment::new(Hand::Right, Finger::Index)
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
}
