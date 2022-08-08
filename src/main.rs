use std::process;

mod game;
use game::Game;

mod error;

fn main() {
    let mut game = Game::new();
    game.render();

    while game.is_running() {
        match game.grab_input() {
            Err(_) => {
                println!("Couldn't get input");
                process::exit(1);
            },
            _ => {}
        }
        game.update();
        game.render();
    }
}
