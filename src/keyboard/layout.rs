use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyboardLayout {
    pub name: String,
    pub rows: Vec<Vec<char>>,
}

impl KeyboardLayout {
    pub fn qwerty() -> Self {
        Self {
            name: "QWERTY".to_string(),
            rows: vec![
                vec!['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
                vec!['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
                vec!['z', 'x', 'c', 'v', 'b', 'n', 'm'],
            ],
        }
    }

    #[allow(dead_code)]
    pub fn dvorak() -> Self {
        Self {
            name: "Dvorak".to_string(),
            rows: vec![
                vec!['\'', ',', '.', 'p', 'y', 'f', 'g', 'c', 'r', 'l'],
                vec!['a', 'o', 'e', 'u', 'i', 'd', 'h', 't', 'n', 's'],
                vec![';', 'q', 'j', 'k', 'x', 'b', 'm', 'w', 'v', 'z'],
            ],
        }
    }

    #[allow(dead_code)]
    pub fn colemak() -> Self {
        Self {
            name: "Colemak".to_string(),
            rows: vec![
                vec!['q', 'w', 'f', 'p', 'g', 'j', 'l', 'u', 'y'],
                vec!['a', 'r', 's', 't', 'd', 'h', 'n', 'e', 'i', 'o'],
                vec!['z', 'x', 'c', 'v', 'b', 'k', 'm'],
            ],
        }
    }
}

impl Default for KeyboardLayout {
    fn default() -> Self {
        Self::qwerty()
    }
}
