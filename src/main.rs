extern crate env_logger;
extern crate pgn_reader;
extern crate shakmaty;
extern crate uci;

use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::Board;
use shakmaty::CastlingSide;
use shakmaty::Chess;
use shakmaty::File;
use shakmaty::Piece;
use shakmaty::Position;
use shakmaty::Rank;
use shakmaty::Role;
use shakmaty::Square;
use uci::Engine;

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
            role,
            file: Some(from.file()),
            rank: Some(from.rank()),
            capture: game.board().role_at(to).is_some(),
            to,
            promotion,
        }
    }
}

fn piece_to_char(p: Piece) -> char {
    p.color.fold_wb(
        match p.role {
            Role::Pawn => '♟',
            Role::Knight => '♞',
            Role::Bishop => '♝',
            Role::Rook => '♜',
            Role::Queen => '♛',
            Role::King => '♚',
        },
        match p.role {
            Role::Pawn => '♙',
            Role::Knight => '♘',
            Role::Bishop => '♗',
            Role::Rook => '♖',
            Role::Queen => '♕',
            Role::King => '♔',
        },
    )
}

fn print_board(board: &Board) {
    for rank in Rank::ALL.iter().rev() {
        for file in File::ALL {
            let square = Square::from_coords(file, *rank);
            print!("{}", board.piece_at(square).map_or('.', piece_to_char));
            print!("{}", if file < File::H { ' ' } else { '\n' });
        }
    }
}

fn main() {
    env_logger::init();

    let engine = Engine::new("stockfish").unwrap().movetime(50);
    let mut game = Chess::default();
    let mut moves = vec![];

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        match San::from_str(input.trim()) {
            Err(_) => {
                eprintln!("Invalid move. Try again");
                continue;
            }
            Ok(san) => match san.to_move(&game) {
                Err(_) => {
                    eprintln!("Illegal move. Try again");
                    continue;
                }
                Ok(mov) => {
                    game = game.play(&mov).unwrap();
                    moves.push(format!("{}{}", mov.from().unwrap(), mov.to()));
                    print_board(game.board());
                }
            },
        }

        engine
            .set_position(
                Fen::from_position(game.clone(), shakmaty::EnPassantMode::Always)
                    .to_string()
                    .as_ref(),
            )
            .unwrap();
        let engine_move_str = engine.bestmove().unwrap();
        println!("{}", engine_move_str);
        let engine_move = parse_move(&engine_move_str, &game).to_move(&game).unwrap();
        game = game.play(&engine_move).unwrap();
        print_board(game.board());
        moves.push(engine_move_str);
    }
}
