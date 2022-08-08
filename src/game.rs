use std::{
    io::{self, stdin, Write},
    ops::{Index, IndexMut},
};

use bitvec::{bitvec, vec::BitVec};
use rand::{seq::SliceRandom, thread_rng};

use self::GameState::*;
use crate::error::{GameError, Result};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GameState {
    /// Show the welcome screen
    Welcome,
    /// Prompt the user to set the size of the board
    SetDimensions,
    /// Prompt the user to pick a card to reveal
    Guess,
    /// Provide feedback about a correct guess
    CorrectGuessConfirm,
    /// Provide feedback about an incorrect guess
    IncorrectGuessConfirm,
    /// Show the stats and prompt for input
    Victory,
    /// End the game
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Card(char);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Vec2 {
    pub x: i32,
    pub y: i32,
}

/// An object used to convert coordinates (2D index) into an array (1D) index.
struct Idx2d {
    pub size_x: i32,
    pub size_y: i32,
}

impl Idx2d {
    /// Create a new indexer object with the given column/row counts.
    pub fn new(size_x: i32, size_y: i32) -> Idx2d {
        Idx2d { size_x, size_y }
    }

    /// Convert coordinates into an array index with bounds checking. If
    /// the coordinates don't map to an array element defined by the stored
    /// sizes, return an `Err`.
    pub fn of(&self, coords: Vec2) -> Result<usize> {
        let Vec2 { x, y } = coords;
        if x < 0 {
            return Err(GameError::CoordinateUnderflow { axis: 'x' });
        }
        if y < 0 {
            return Err(GameError::CoordinateUnderflow { axis: 'y' });
        }
        if x >= self.size_x {
            return Err(GameError::CoordinateOverflow {
                axis: 'x',
                max: self.size_x,
            });
        }
        if y >= self.size_y {
            return Err(GameError::CoordinateOverflow {
                axis: 'y',
                max: self.size_y,
            });
        }
        Ok(self.unchecked(coords))
    }

    /// Convert coordinates into an array index without bounds checking.
    pub fn unchecked(&self, coords: Vec2) -> usize {
        let Vec2 { x, y } = coords;
        (y * self.size_x + x) as usize
    }

    /// Iterate through all the possible coordinates - defined by `size_x`
    /// and `size_y` - in row major order.
    pub fn iter_all(&self) -> impl Iterator<Item = Vec2> + '_ {
        let max = self.size_x * self.size_y;
        (0..max).map(|i| {
            let x = i % self.size_x;
            let y = i / self.size_x;
            Vec2 { x, y }
        })
    }
}

/// A board of playing cards
struct Board {
    idx: Idx2d,
    pub cards: Vec<Card>,
}

impl Board {
    /// Symbols to use as "cards"
    const CARD_CHARS: [char; 55] = [
        '☀', '☁', '★', '☇', '☈', '☉', '☊', '☋', '☌', '☍', '☎', '☔', '☕', '☗',
        '☘', '☙', '☚', '☛', '☝', '☠', '☡', '☢', '☣', '☤', '☥', '☦', '☧', '☩',
        '☫', '☬', '☭', '☮', '☯', '☼', '☿', '♀', '♁', '♂', '♃', '♄', '♅', '♆',
        '♇', '♈', '♉', '♊', '♋', '♌', '♍', '♎', '♏', '♐', '♑', '♒',
        '♓',
    ];

    /// Maximum possible board size
    const MAX_SIZE: i32 = (Board::CARD_CHARS.len() * 2) as i32;

    /// Create a new board with the given sizes and fill it randomly with cards
    /// from the [predefined list](`Board::CARD_CHARS`).
    pub fn new(size_x: i32, size_y: i32) -> Result<Board> {
        debug_assert!(size_x > 0);
        debug_assert!(size_y > 0);
        debug_assert!((size_x * size_y) % 2 == 0);

        let size = (size_x * size_y) as usize;
        let mut board = Board {
            idx: Idx2d::new(size_x, size_y),
            cards: vec![Card('\0'); size],
        };

        // Find all coordinates of all the available spaces
        let mut rng = thread_rng();
        let mut coords: Vec<_> = board.idx.iter_all().collect();
        coords.shuffle(&mut rng);

        // Length is always even, OK to split in two
        let chunk_size = coords.len() / 2;
        let [first_half, second_half]: [&[Vec2]; 2] = coords
            .chunks(chunk_size)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        // Assign a card to each pair of spaces
        for (i, (c1, c2)) in first_half.iter().zip(second_half).enumerate() {
            let card = Card(Board::CARD_CHARS[i]);

            let cell1 = &mut board[*c1];
            *cell1 = card;

            let cell2 = &mut board[*c2];
            *cell2 = card;
        }

        Ok(board)
    }

    /// Create an empty board with 0 size.
    pub fn default() -> Board {
        Board {
            idx: Idx2d::new(0, 0),
            cards: Vec::new(),
        }
    }
}

impl Index<Vec2> for Board {
    type Output = Card;

    fn index(&self, index: Vec2) -> &Self::Output {
        &self.cards[self.idx.unchecked(index)]
    }
}

impl IndexMut<Vec2> for Board {
    fn index_mut(&mut self, index: Vec2) -> &mut Self::Output {
        &mut self.cards[self.idx.unchecked(index)]
    }
}

/// A card matching game.
pub struct Game {
    /// The game state.
    state: GameState,
    /// The last user input.
    user_input: String,
    /// Number of guesses by the user.
    guesses: i32,
    /// 2D-to-1D index converter utility.
    idx: Idx2d,
    /// The game board.
    board: Board,
    /// List of flags corresponding to elements in [`Board::cards`].
    /// A set bit in a given position indicates that a card has been
    /// succesfully matched.
    discovered: BitVec,
    /// One of the cards revealed by the user during the guessing phase.
    revealed1: Option<Vec2>,
    /// One of the cards revealed by the user during the guessing phase.
    /// Always revealed after [`Game::revealed1`]
    revealed2: Option<Vec2>,
    /// An error encountered during user input parsing.
    error: Option<GameError>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            state: Welcome,
            user_input: String::new(),
            guesses: 0,
            idx: Idx2d::new(0, 0),
            board: Board::default(),
            discovered: bitvec![0; 0],
            revealed1: None,
            revealed2: None,
            error: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state != Exit
    }

    /// Read input from `stdin`.
    pub fn grab_input(&mut self) -> io::Result<()> {
        self.user_input.clear();
        stdin().read_line(&mut self.user_input)?;
        Ok(())
    }

    /// Update the game based on the latest result from [`Game::grab_input`].
    pub fn update(&mut self) {
        self.error = None;

        match self.state {
            Welcome => self.state = SetDimensions,
            SetDimensions => match self.set_dimensions() {
                Ok(_) => self.state = Guess,
                Err(e) => self.error = Some(e),
            },
            Guess => {
                let c = match self.parse_coords(&self.user_input) {
                    Ok(c) => c,
                    Err(e) => {
                        self.error = Some(e);
                        return;
                    }
                };
                if self.is_revealed(c) || self.is_discovered(c) {
                    self.error = Some(GameError::AlreadyRevealed {
                        x: c.x + 1,
                        y: c.y + 1,
                    });
                    return;
                }
                if self.can_reveal() {
                    self.set_revealed(c);
                }
                if !self.can_reveal() {
                    if self.revealed_match() {
                        self.state = CorrectGuessConfirm;
                    } else {
                        self.state = IncorrectGuessConfirm;
                    }
                }
            }
            CorrectGuessConfirm => {
                self.set_discovered(self.revealed1.unwrap());
                self.set_discovered(self.revealed2.unwrap());
                self.inc_guesses();
                self.clear_revealed();

                if self.all_discovered() {
                    self.state = Victory
                } else {
                    self.state = Guess;
                }
            }
            IncorrectGuessConfirm => {
                self.inc_guesses();
                self.clear_revealed();
                self.state = Guess;
            }
            Victory => match self.parse_yn(&self.user_input) {
                Ok(true) => self.state = SetDimensions,
                Ok(false) => self.state = Exit,
                Err(e) => self.error = Some(e),
            },
            _ => {}
        }
    }

    /// Render the current state.
    pub fn render(&self) {
        self.render_clear();

        match self.state {
            Welcome => {
                println!("Welcome! Press <Enter> to begin.");
            }
            SetDimensions => {
                self.render_error();
                println!("Set board dimensions (x, y)");
                print!("> ");
                io::stdout().flush().unwrap();
            }
            Guess => {
                self.render_score();
                self.render_board();
                self.render_error();
                println!("Pick a card (x, y)");
                print!("> ");
                io::stdout().flush().unwrap();
            }
            CorrectGuessConfirm => {
                self.render_score();
                self.render_board();
                println!("A match!");
            }
            IncorrectGuessConfirm => {
                self.render_score();
                self.render_board();
                println!("Try again")
            }
            Victory => {
                self.render_score();
                self.render_board();
                self.render_error();
                println!("Congratulations! Play again? (y / N)");
                print!("> ");
                io::stdout().flush().unwrap();
            }
            _ => {}
        }
    }

    /// Attempt to parse a pair of i32 numbers from the string slice.
    /// Accepts `x,y` and `x;y` formats with any amount of whitespace.
    fn parse_pair(s: &str) -> Result<Vec2> {
        if s.is_empty() {
            return Err(GameError::EmptyInput);
        }

        let parts: Vec<_> = s
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim())
            .collect();

        if parts.len() < 2 {
            return Err(GameError::UnparsableInput);
        }

        let x = parts[0]
            .parse::<i32>()
            .map_err(|_| GameError::UnparsableInput)?;

        let y = parts[1]
            .parse::<i32>()
            .map_err(|_| GameError::UnparsableInput)?;

        Ok(Vec2 { x, y })
    }

    /// Attempt to interpret the string slice as the size of the game board.
    fn parse_dimensions(&self, s: &str) -> Result<Vec2> {
        let p = Game::parse_pair(s)?;

        if p.x <= 0 {
            return Err(GameError::CoordinateUnderflow { axis: 'x' });
        }
        if p.y <= 0 {
            return Err(GameError::CoordinateUnderflow { axis: 'y' });
        }

        // Cannot display more kinds of cards than those defined in the
        // CARD_CHARS array
        if p.x * p.y > Board::MAX_SIZE {
            return Err(GameError::NotEnoughCardTypes {
                max: Board::MAX_SIZE,
            });
        }

        if (p.x * p.y) % 2 != 0 {
            return Err(GameError::OddBoardCells);
        }

        Ok(p)
    }

    /// Attempt to interpret the string slice as the position of a card on
    /// the game board.
    fn parse_coords(&self, s: &str) -> Result<Vec2> {
        let p = Game::parse_pair(s)?;
        let coords = Vec2 {
            x: p.x - 1,
            y: p.y - 1,
        };
        self.idx.of(coords)?;
        Ok(coords)
    }

    /// Parse a yes/no response from the string slice. Defaults to `false`.
    fn parse_yn(&self, s: &str) -> Result<bool> {
        match s.to_lowercase().trim() {
            "y" => Ok(true),
            "n" => Ok(false),
            s if s.is_empty() => Ok(false),
            _ => Err(GameError::UnparsableInput),
        }
    }

    /// Attempt to create a new board from the latest user input and prepare
    /// for the game to begin.
    fn set_dimensions(&mut self) -> Result<()> {
        debug_assert!(!self.user_input.is_empty());

        let Vec2 { x, y } = self.parse_dimensions(&self.user_input)?;
        self.idx = Idx2d::new(x, y);
        self.discovered = bitvec![0; (x * y) as usize];
        self.board = Board::new(x, y)?;
        Ok(())
    }

    /// Mark a position as having been correctly matched.
    fn set_discovered(&mut self, c: Vec2) {
        let index = self.idx.unchecked(c);
        self.discovered.set(index, true);
    }

    /// Check if a given position has been correctly matched.
    fn is_discovered(&self, c: Vec2) -> bool {
        let index = self.idx.unchecked(c);
        *self.discovered.get(index).unwrap()
    }

    /// Check if all cards have been correctly matched.
    fn all_discovered(&self) -> bool {
        self.discovered.count_ones() == self.discovered.len()
    }

    /// Check if it's possible to reveal a card during the current
    /// guess phase.
    fn can_reveal(&self) -> bool {
        self.revealed1.is_none() || self.revealed2.is_none()
    }

    /// Check if the two cards revealed during the guess phase match.
    /// # Panics
    /// Panics if one or both of [`Game::revealed1`] and [`Game::revealed2`] was
    /// not set.
    fn revealed_match(&self) -> bool {
        let r1 = self.board[self.revealed1.unwrap()];
        let r2 = self.board[self.revealed2.unwrap()];
        r1 == r2
    }

    /// Mark a card as revealed during the guess phase.
    fn set_revealed(&mut self, c: Vec2) {
        if self.revealed1.is_none() {
            self.revealed1 = Some(c);
        } else {
            self.revealed2 = Some(c);
        }
    }

    /// Clear both revealed cards.
    fn clear_revealed(&mut self) {
        self.revealed1 = None;
        self.revealed2 = None;
    }

    /// Check if a card at a given position has been revealed during the
    /// guessing phase.
    fn is_revealed(&self, c: Vec2) -> bool {
        matches!(self.revealed1, Some(x) if c == x)
            || matches!(self.revealed2, Some(x) if c == x)
    }

    /// Increment the number of guesses.
    fn inc_guesses(&mut self) {
        self.guesses += 1;
    }

    /// Clear the screen.
    fn render_clear(&self) {
        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    }

    /// Render the cards and reveal indicators.
    fn render_board(&self) {
        let mut board_img: Vec<char> = vec![];
        for coords in self.idx.iter_all() {
            if self.is_discovered(coords) {
                board_img.push(self.board[coords].0);
                board_img.push(' ');
                board_img.push(' ');
            } else if self.is_revealed(coords) {
                board_img.push(self.board[coords].0);
                board_img.push(' ');
                board_img.push('<');
            } else {
                board_img.push('█');
                board_img.push(' ');
                board_img.push(' ');
            }

            if coords.x == self.idx.size_x - 1 {
                board_img.push('\n');
                board_img.push('\n');
            }
        }
        let board_img: String = board_img.iter().collect();
        println!("{}", board_img);
    }

    /// Render the error message, if there is one.
    fn render_error(&self) {
        if let Some(err) = &self.error {
            println!("(!) {}", err.as_string());
        }
    }

    /// Render the total and correct number of guesses.
    fn render_score(&self) {
        let correct_guesses = self.discovered.count_ones() / 2;
        println!(
            "Guesses: {} | Correct guesses: {}\n",
            self.guesses, correct_guesses
        );
    }
}
