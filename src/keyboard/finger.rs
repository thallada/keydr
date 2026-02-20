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

    pub fn description(&self) -> &'static str {
        match (self.hand, self.finger) {
            (Hand::Left, Finger::Pinky) => "left pinky",
            (Hand::Left, Finger::Ring) => "left ring finger",
            (Hand::Left, Finger::Middle) => "left middle finger",
            (Hand::Left, Finger::Index) => "left index finger",
            (Hand::Left, Finger::Thumb) => "left thumb",
            (Hand::Right, Finger::Pinky) => "right pinky",
            (Hand::Right, Finger::Ring) => "right ring finger",
            (Hand::Right, Finger::Middle) => "right middle finger",
            (Hand::Right, Finger::Index) => "right index finger",
            (Hand::Right, Finger::Thumb) => "right thumb",
        }
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
