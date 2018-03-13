extern crate shakmaty;
extern crate uci;
extern crate env_logger;
extern crate pgn_reader;

use uci::Engine;
use shakmaty::Chess;
use shakmaty::san::San;
use shakmaty::Square;
use shakmaty::Role;
use shakmaty::Position;
use shakmaty::CastlingSide;
use shakmaty::Setup;

use std::io::stdin;
use std::str::FromStr;

fn parse_move(s: &str, game: &Chess) -> San {
    let from: Square = s[..2].parse().unwrap();
    let to: Square = s[2..4].parse().unwrap();
    let promotion = if s.len() > 4 {
        Role::from_char(s.chars().nth(4).unwrap())
    } else {
        None
    };
    let role = game.board().role_at(from).unwrap();
    if role == Role::King {
        if from.file() - to.file() == 2 {
            San::Castle(CastlingSide::KingSide)
        } else {
            San::Castle(CastlingSide::QueenSide)
        }
    } else {
        San::Normal {
            role: role,
            file: Some(from.file()),
            rank: Some(from.rank()),
            capture: ! game.board().role_at(to).is_none(),
            to: to,
            promotion: promotion,
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let engine = Engine::new("stockfish").unwrap().movetime(50);
    let mut game = Chess::default();
    let mut moves = vec![];

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        let mov = San::from_str(input.trim()).unwrap().to_move(&game).unwrap();
        game = game.play(&mov).unwrap();

        moves.push(format!("{}{}", mov.from().unwrap(), mov.to()));
        println!("{:?}", game.board());

        engine.make_moves(&moves).unwrap();
        let engine_move_str = engine.bestmove().unwrap();
        println!("{}", engine_move_str);
        let engine_move = parse_move(&engine_move_str, &game).to_move(&game).unwrap();
        game = game.play(&engine_move).unwrap();
        println!("{:?}", game.board());
        moves.push(engine_move_str);
    }
}
