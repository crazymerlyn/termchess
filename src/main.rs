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

fn normalize_san(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("o-o") {
        return "O-O".to_string();
    }
    if trimmed.eq_ignore_ascii_case("o-o-o") {
        return "O-O-O".to_string();
    }
    let mut chars: Vec<char> = trimmed.chars().collect();
    if let Some(first) = chars.first_mut() {
        match first {
            'n' | 'r' | 'q' | 'k' => *first = first.to_ascii_uppercase(),
            _ => {}
        }
    }
    for i in 0..chars.len().saturating_sub(1) {
        if chars[i] == '=' {
            chars[i + 1] = chars[i + 1].to_ascii_uppercase();
            break;
        }
    }
    chars.into_iter().collect()
}

fn parse_move(s: &str, game: &Chess) -> Option<San> {
    if s.len() < 4 {
        return None;
    }
    let from: Square = s[..2].parse().ok()?;
    let to: Square = s[2..4].parse().ok()?;
    let promotion = if s.len() > 4 {
        Role::from_char(s.chars().nth(4)?)
    } else {
        None
    };
    let role = game.board().role_at(from)?;
    if role == Role::King {
        let file_diff: i32 = from.file() - to.file();
        if file_diff == 2 {
            Some(San::Castle(CastlingSide::QueenSide))
        } else if file_diff == -2 {
            Some(San::Castle(CastlingSide::KingSide))
        } else {
            Some(San::Normal {
                role,
                file: Some(from.file()),
                rank: Some(from.rank()),
                capture: game.board().role_at(to).is_some(),
                to,
                promotion,
            })
        }
    } else {
        Some(San::Normal {
            role,
            file: Some(from.file()),
            rank: Some(from.rank()),
            capture: game.board().role_at(to).is_some(),
            to,
            promotion,
        })
    }
}

fn piece_to_char(p: Piece) -> char {
    p.color.fold_wb(
        match p.role {
            Role::Pawn => '♙',
            Role::Knight => '♘',
            Role::Bishop => '♗',
            Role::Rook => '♖',
            Role::Queen => '♕',
            Role::King => '♔',
        },
        match p.role {
            Role::Pawn => '♟',
            Role::Knight => '♞',
            Role::Bishop => '♝',
            Role::Rook => '♜',
            Role::Queen => '♛',
            Role::King => '♚',
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
    let _ = stdout().flush();
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
        match stdin().read_line(&mut input) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        match San::from_str(&normalize_san(&input)) {
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

                    if let Some(outcome) = game.outcome() {
                        render(game.board(), &moves, &format!("Game over! {}", outcome));
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

                    let engine_san = match parse_move(&engine_move_str, &game) {
                        None => {
                            render(
                                game.board(),
                                &moves,
                                &format!("Engine error: illegal move {}", engine_move_str),
                            );
                            break;
                        }
                        Some(s) => s,
                    };
                    let engine_move = match engine_san.to_move(&game) {
                        Err(_) => {
                            render(
                                game.board(),
                                &moves,
                                &format!("Engine error: illegal move {}", engine_move_str),
                            );
                            break;
                        }
                        Ok(m) => m,
                    };
                    let san_str = San::from_move(&game, &engine_move).to_string();
                    match game.play(&engine_move) {
                        Err(_) => {
                            eprintln!("Engine error: could not play {}", engine_move_str);
                            break;
                        }
                        Ok(new_game) => {
                            game = new_game;
                            moves.push(san_str);

                            if let Some(outcome) = game.outcome() {
                                render(game.board(), &moves, &format!("Game over! {}", outcome));
                                break;
                            }
                        }
                    }

                    let side = if game.turn().is_white() { "White" } else { "Black" };
                    render(game.board(), &moves, &format!("{} to move:", side));
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::Color;

    #[test]
    fn test_normalize_san_piece_initials() {
        assert_eq!(normalize_san("nf3"), "Nf3");
        assert_eq!(normalize_san("rc3"), "Rc3");
        assert_eq!(normalize_san("qc3"), "Qc3");
        assert_eq!(normalize_san("kc3"), "Kc3");
    }

    #[test]
    fn test_normalize_san_already_uppercase() {
        assert_eq!(normalize_san("Nf3"), "Nf3");
        assert_eq!(normalize_san("Rc3"), "Rc3");
    }

    #[test]
    fn test_normalize_san_castling() {
        assert_eq!(normalize_san("o-o"), "O-O");
        assert_eq!(normalize_san("O-O"), "O-O");
        assert_eq!(normalize_san("o-o-o"), "O-O-O");
        assert_eq!(normalize_san("O-O-O"), "O-O-O");
    }

    #[test]
    fn test_normalize_san_pawn_moves() {
        assert_eq!(normalize_san("e4"), "e4");
        assert_eq!(normalize_san("d5"), "d5");
        assert_eq!(normalize_san("exd5"), "exd5");
    }

    #[test]
    fn test_normalize_san_promotion() {
        assert_eq!(normalize_san("e8=q"), "e8=Q");
        assert_eq!(normalize_san("e8=Q"), "e8=Q");
    }

    #[test]
    fn test_normalize_san_ambiguous_b() {
        assert_eq!(normalize_san("bc3"), "bc3");
        assert_eq!(normalize_san("Bc3"), "Bc3");
    }

    #[test]
    fn test_normalize_san_trims_whitespace() {
        assert_eq!(normalize_san("  nf3  "), "Nf3");
    }

    #[test]
    fn test_piece_to_char() {
        let w = Piece { color: Color::White, role: Role::Pawn };
        assert_eq!(piece_to_char(w), '♙');
        let w = Piece { color: Color::White, role: Role::Knight };
        assert_eq!(piece_to_char(w), '♘');
        let w = Piece { color: Color::White, role: Role::Bishop };
        assert_eq!(piece_to_char(w), '♗');
        let w = Piece { color: Color::White, role: Role::Rook };
        assert_eq!(piece_to_char(w), '♖');
        let w = Piece { color: Color::White, role: Role::Queen };
        assert_eq!(piece_to_char(w), '♕');
        let w = Piece { color: Color::White, role: Role::King };
        assert_eq!(piece_to_char(w), '♔');

        let b = Piece { color: Color::Black, role: Role::Pawn };
        assert_eq!(piece_to_char(b), '♟');
        let b = Piece { color: Color::Black, role: Role::Knight };
        assert_eq!(piece_to_char(b), '♞');
        let b = Piece { color: Color::Black, role: Role::Bishop };
        assert_eq!(piece_to_char(b), '♝');
        let b = Piece { color: Color::Black, role: Role::Rook };
        assert_eq!(piece_to_char(b), '♜');
        let b = Piece { color: Color::Black, role: Role::Queen };
        assert_eq!(piece_to_char(b), '♛');
        let b = Piece { color: Color::Black, role: Role::King };
        assert_eq!(piece_to_char(b), '♚');
    }

    #[test]
    fn test_parse_move_valid() {
        let game = Chess::default();
        let san = parse_move("e2e4", &game).expect("should parse e2e4");
        assert!(san.to_move(&game).is_ok());
    }

    #[test]
    fn test_parse_move_short_input() {
        let game = Chess::default();
        assert!(parse_move("e4", &game).is_none());
        assert!(parse_move("", &game).is_none());
    }

    #[test]
    fn test_parse_move_invalid_square() {
        let game = Chess::default();
        assert!(parse_move("e9e5", &game).is_none());
    }

    #[test]
    fn test_parse_move_no_piece_at_source() {
        let game = Chess::default();
        assert!(parse_move("e4e5", &game).is_none());
    }

    #[test]
    fn test_parse_move_castling_detection() {
        let game = Chess::default();
        match parse_move("e1g1", &game).expect("should parse e1g1") {
            San::Castle(CastlingSide::KingSide) => {}
            _ => panic!("e1g1 should be king-side castling"),
        }
        match parse_move("e1c1", &game).expect("should parse e1c1") {
            San::Castle(CastlingSide::QueenSide) => {}
            _ => panic!("e1c1 should be queen-side castling"),
        }
        match parse_move("e8g8", &game).expect("should parse e8g8") {
            San::Castle(CastlingSide::KingSide) => {}
            _ => panic!("e8g8 should be king-side castling"),
        }
        match parse_move("e8c8", &game).expect("should parse e8c8") {
            San::Castle(CastlingSide::QueenSide) => {}
            _ => panic!("e8c8 should be queen-side castling"),
        }
    }
}
