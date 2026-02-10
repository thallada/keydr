#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FingerAssignment {
    pub hand: Hand,
    pub finger: Finger,
}

impl FingerAssignment {
    pub fn new(hand: Hand, finger: Finger) -> Self {
        Self { hand, finger }
    }
}

#[allow(dead_code)]
pub fn qwerty_finger(ch: char) -> FingerAssignment {
    use Finger::*;
    use Hand::*;

    match ch {
        'q' | 'a' | 'z' | '1' => FingerAssignment::new(Left, Pinky),
        'w' | 's' | 'x' | '2' => FingerAssignment::new(Left, Ring),
        'e' | 'd' | 'c' | '3' => FingerAssignment::new(Left, Middle),
        'r' | 'f' | 'v' | 't' | 'g' | 'b' | '4' | '5' => FingerAssignment::new(Left, Index),
        'y' | 'h' | 'n' | 'u' | 'j' | 'm' | '6' | '7' => FingerAssignment::new(Right, Index),
        'i' | 'k' | ',' | '8' => FingerAssignment::new(Right, Middle),
        'o' | 'l' | '.' | '9' => FingerAssignment::new(Right, Ring),
        'p' | ';' | '/' | '0' | '-' | '=' | '[' | ']' | '\'' | '\\' => {
            FingerAssignment::new(Right, Pinky)
        }
        ' ' => FingerAssignment::new(Right, Thumb),
        _ => FingerAssignment::new(Right, Index),
    }
}
