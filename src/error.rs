pub type Result<T> = std::result::Result<T, GameError>;

#[derive(Clone, Debug)]
pub enum GameError {
    /// Tried to reveal a card that was already revealed or matched.
    AlreadyRevealed { x: i32, y: i32 },
    /// Supplied empty input.
    EmptyInput,
    /// Supplied a coordinate beyond the maximum bound of the board.
    CoordinateOverflow { axis: char, max: i32 },
    /// Supplied a coordinate below the minimum bound of the board.
    CoordinateUnderflow { axis: char },
    /// Requested too many board spaces to be created.
    NotEnoughCardTypes { max: i32 },
    /// Requested an odd number of board spaces to be created.
    OddBoardCells,
    /// Supplied input that we were unable to interpret.
    UnparsableInput,
}

impl GameError {
    pub fn as_string(&self) -> String {
        use GameError::*;

        let message = match self {
            AlreadyRevealed { x, y } => {
                format!("Card at position ({},{}) is already revealed.", x, y)
            }
            EmptyInput => {
                "User input is required".to_owned()
            }
            CoordinateOverflow { axis, max } => {
                format!("{} coordinate too large. Maximum possible value is {}.", axis, max)
            }
            CoordinateUnderflow { axis } => {
                format!("{} coordinate too small. Minimum possible value is 0.", axis)
            }
            OddBoardCells => {
                "Number of board cells (horizontal size * vertical size) must be even".to_owned()
            }
            NotEnoughCardTypes { max } => {
                format!("Cannot create board with more than {} cells", max * 2)
            }
            UnparsableInput => {
                "User input could not be parsed".to_owned()
            }
        };

        return message;
    }
}
