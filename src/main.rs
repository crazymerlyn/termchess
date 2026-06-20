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

use std::io::{stdout, Write, stdin};
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
        let file_diff: i32 = from.file() - to.file();
        if file_diff == 2 {
            San::Castle(CastlingSide::QueenSide)
        } else if file_diff == -2 {
            San::Castle(CastlingSide::KingSide)
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

fn render(board: &Board, moves: &[String], status: &str) {
    print!("\x1b[2J\x1b[H");
    println!("  {}", status);
    println!();
    print!("   ");
    for f in File::ALL {
        print!(" {} ", f.char());
    }
    println!();
    for rank in Rank::ALL.iter().rev() {
        print!(" {} ", rank.char());
        for file in File::ALL {
            let square = Square::from_coords(file, *rank);
            match board.piece_at(square) {
                Some(p) => print!(" {} ", piece_to_char(p)),
                None => print!(" · "),
            }
        }
        println!(" {}", rank.char());
    }
    print!("   ");
    for f in File::ALL {
        print!(" {} ", f.char());
    }
    println!();
    println!();
    if !moves.is_empty() {
        println!("  Moves:");
        for (i, chunk) in moves.chunks(2).enumerate() {
            let w = chunk.first().map(|s| s.as_str()).unwrap_or("");
            let b = chunk.get(1).map(|s| s.as_str()).unwrap_or("");
            println!("  {}. {:>7}  {:>7}", i + 1, w, b);
        }
        println!();
    }
    print!("  Your move: ");
    stdout().flush().unwrap();
}

fn main() {
    let engine = match Engine::new("stockfish") {
        Ok(eng) => eng.movetime(500),
        Err(e) => {
            eprintln!("Failed to start Stockfish: {}. Is it installed and on your PATH?", e);
            return;
        }
    };
    let mut game = Chess::default();
    let mut moves: Vec<String> = vec![];

    render(game.board(), &moves, "Welcome to termchess!");

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        match San::from_str(input.trim()) {
            Err(_) => {
                render(game.board(), &moves, "Invalid move. Try again:");
                continue;
            }
            Ok(san) => match san.to_move(&game) {
                Err(_) => {
                    render(game.board(), &moves, "Illegal move. Try again:");
                    continue;
                }
                Ok(mov) => {
                    let san_str = San::from_move(&game, &mov).to_string();
                    game = game.play(&mov).unwrap();
                    moves.push(san_str);

                    if game.outcome().is_some() {
                        render(game.board(), &moves, "Game over!");
                        break;
                    }

                    render(game.board(), &moves, "Thinking...");

                    engine
                        .set_position(
                            Fen::from_position(game.clone(), shakmaty::EnPassantMode::Always)
                                .to_string()
                                .as_ref(),
                        )
                        .unwrap();
                    let engine_move_str = engine.bestmove().unwrap();

                    match parse_move(&engine_move_str, &game).to_move(&game) {
                        Err(_) => {
                            render(
                                game.board(),
                                &moves,
                                &format!("Engine error: illegal move {}", engine_move_str),
                            );
                            break;
                        }
                        Ok(engine_move) => {
                            let engine_san = San::from_move(&game, &engine_move).to_string();
                            match game.play(&engine_move) {
                                Err(_) => {
                                    eprintln!("Engine error: could not play {}", engine_move_str);
                                    break;
                                }
                                Ok(new_game) => {
                                    game = new_game;
                                    moves.push(engine_san);

                                    if game.outcome().is_some() {
                                        render(game.board(), &moves, "Game over!");
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    render(game.board(), &moves, "Your turn:");
                }
            },
        }
    }
}
