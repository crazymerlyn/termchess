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

use std::io::{stdin, stdout, Write};
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

fn normal_san(role: Role, from: Square, to: Square, promotion: Option<Role>, capture: bool) -> San {
    San::Normal {
        role,
        file: Some(from.file()),
        rank: Some(from.rank()),
        capture,
        to,
        promotion,
    }
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
    let capture = game.board().role_at(to).is_some();
    if role == Role::King {
        let file_diff: i32 = from.file() - to.file();
        if file_diff == 2 {
            return Some(San::Castle(CastlingSide::QueenSide));
        }
        if file_diff == -2 {
            return Some(San::Castle(CastlingSide::KingSide));
        }
    }
    Some(normal_san(role, from, to, promotion, capture))
}

fn piece_to_char(p: Piece) -> String {
    let ch = p.color.fold_wb(
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
    );
    if p.color.is_white() {
        format!("\x1b[97m{}", ch)
    } else {
        format!("\x1b[31m{}", ch)
    }
}

fn board_fen(game: &Chess) -> String {
    Fen::from_position(game.clone(), shakmaty::EnPassantMode::Always).to_string()
}

fn apply_move(
    game: &mut Chess,
    mov: &shakmaty::Move,
    halfmove_clock: &mut u32,
    moves: &mut Vec<String>,
    positions: &mut Vec<String>,
    game_states: &mut Vec<Chess>,
    clock_history: &mut Vec<u32>,
) -> Result<(), String> {
    let san_str = San::from_move(game, mov).to_string();
    let role = game.board().role_at(mov.from().unwrap());
    let capture = game.board().role_at(mov.to()).is_some()
        || game.ep_square(shakmaty::EnPassantMode::Always) == Some(mov.to());
    match game.clone().play(mov) {
        Err(_) => return Err("Internal error: failed to play move".to_string()),
        Ok(new_game) => *game = new_game,
    }
    if role == Some(Role::Pawn) || capture {
        *halfmove_clock = 0;
    } else {
        *halfmove_clock += 1;
    }
    moves.push(san_str);
    positions.push(board_fen(game));
    game_states.push(game.clone());
    clock_history.push(*halfmove_clock);
    Ok(())
}

fn play_engine_move(
    engine: &Engine,
    game: &mut Chess,
    halfmove_clock: &mut u32,
    moves: &mut Vec<String>,
    positions: &mut Vec<String>,
    game_states: &mut Vec<Chess>,
    clock_history: &mut Vec<u32>,
) -> Result<shakmaty::Move, String> {
    let fen = board_fen(game);
    engine.set_position(&fen).map_err(|_| "Engine error: failed to set position".to_string())?;
    let engine_move_str = engine.bestmove().map_err(|_| "Engine error: failed to get best move".to_string())?;
    let engine_san = parse_move(&engine_move_str, game).ok_or_else(|| format!("Engine error: illegal move {}", engine_move_str))?;
    let engine_move = engine_san.to_move(game).map_err(|_| format!("Engine error: illegal move {}", engine_move_str))?;
    apply_move(game, &engine_move, halfmove_clock, moves, positions, game_states, clock_history)
        .map_err(|_| format!("Engine error: could not play {}", engine_move_str))?;
    Ok(engine_move)
}

fn check_draw(positions: &[String], halfmove_clock: u32) -> Option<String> {
    if halfmove_clock >= 100 {
        return Some("Draw by 50-move rule".to_string());
    }
    if let Some(last) = positions.last() {
        if positions.iter().filter(|p| *p == last).count() >= 3 {
            return Some("Draw by threefold repetition".to_string());
        }
    }
    None
}

fn render(board: &Board, moves: &[String], status: &str, last_move: Option<(Square, Square)>, flip: bool) {
    print!("\x1b[2J\x1b[H");
    println!("  {}", status);
    println!();
    print!("   ");
    for f in File::ALL {
        print!(" {} ", f.char());
    }
    println!();
    let ranks: Vec<&Rank> = if flip { Rank::ALL.iter().collect() } else { Rank::ALL.iter().rev().collect() };
    for rank in ranks {
        print!(" {} ", rank.char());
        for file in File::ALL {
            let square = Square::from_coords(file, *rank);
            let hl = last_move.is_some() && (last_move.unwrap().0 == square || last_move.unwrap().1 == square);
            if hl {
                print!("\x1b[43m");
            }
            match board.piece_at(square) {
                Some(p) => print!(" {} ", piece_to_char(p)),
                None => print!("\x1b[90m · \x1b[0m"),
            }
            if hl {
                print!("\x1b[0m");
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
        let total = moves.len();
        let start = total.saturating_sub(32);
        if start > 0 {
            println!("  ...");
        }
        for (i, chunk) in moves[start..].chunks(2).enumerate() {
            let w = chunk.first().map(|s| s.as_str()).unwrap_or("");
            let b = chunk.get(1).map(|s| s.as_str()).unwrap_or("");
            println!("  {}. {:>7}  {:>7}", (start / 2) + i + 1, w, b);
        }
        println!();
    }
    print!("  Your move: ");
    let _ = stdout().flush();
}

struct Config {
    engine_path: String,
    play_as_black: bool,
}

fn parse_args() -> Config {
    let mut engine_path: Option<String> = None;
    let mut play_as_black = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--engine" | "-e" => {
                engine_path = Some(args.next().unwrap_or_else(|| {
                    eprintln!("error: --engine requires a path argument");
                    eprintln!("Try `termchess --help` for more information.");
                    std::process::exit(1);
                }));
            }
            "--side" | "-s" => {
                let side = args.next().unwrap_or_else(|| {
                    eprintln!("error: --side requires an argument (white or black)");
                    eprintln!("Try `termchess --help` for more information.");
                    std::process::exit(1);
                });
                match side.as_str() {
                    "black" => play_as_black = true,
                    "white" => {}
                    _ => {
                        eprintln!("error: invalid side '{}' (use white or black)", side);
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!("Usage: termchess [OPTIONS]");
                println!();
                println!("Play chess against a UCI engine.");
                println!();
                println!("Options:");
                println!("  -e, --engine <PATH>  Path to UCI engine executable");
                println!("                        (default: stockfish, or $STOCKFISH_PATH)");
                println!("  -s, --side <SIDE>    Play as white or black (default: white)");
                println!("  -h, --help           Print this help message");
                println!("  -V, --version        Print version");
                std::process::exit(0);
            }
            "--version" | "-V" => {
                println!("termchess {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => {
                eprintln!("error: unknown flag '{}'", arg);
                eprintln!("Try `termchess --help` for more information.");
                std::process::exit(1);
            }
        }
    }
    Config {
        engine_path: engine_path.unwrap_or_else(|| {
            std::env::var("STOCKFISH_PATH")
                .ok()
                .unwrap_or_else(|| "stockfish".to_string())
        }),
        play_as_black,
    }
}

fn main() {
    let cfg = parse_args();
    let engine = match Engine::new(&cfg.engine_path) {
        Ok(eng) => eng.movetime(500),
        Err(e) => {
            eprintln!("Failed to start engine '{}': {}", cfg.engine_path, e);
            std::process::exit(1);
        }
    };
    let mut game = Chess::default();
    let mut moves: Vec<String> = vec![];
    let mut positions: Vec<String> = vec![board_fen(&game)];
    let mut halfmove_clock: u32 = 0;
    let mut game_states: Vec<Chess> = vec![game.clone()];
    let mut clock_history: Vec<u32> = vec![0];
    let mut last_move: Option<(Square, Square)> = None;
    let flip = cfg.play_as_black;

    if cfg.play_as_black {
        match play_engine_move(&engine, &mut game, &mut halfmove_clock, &mut moves, &mut positions, &mut game_states, &mut clock_history) {
            Ok(m) => {
                last_move = Some((m.from().unwrap(), m.to()));
                render(game.board(), &moves, "Black to move:", last_move, flip);
            }
            Err(e) => {
                render(game.board(), &moves, &e, last_move, flip);
                return;
            }
        }
    } else {
        render(
            game.board(),
            &moves,
            "Welcome to termchess! Enter a move, or type undo/resign/draw.",
            last_move,
            flip,
        );
    }

    loop {
        let mut input = String::new();
        match stdin().read_line(&mut input) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let cmd = input.trim().to_lowercase();
        match cmd.as_str() {
            "undo" | "u" => {
                if moves.len() >= 2 {
                    game_states.truncate(game_states.len() - 2);
                    clock_history.truncate(clock_history.len() - 2);
                    game = game_states.last().unwrap().clone();
                    halfmove_clock = *clock_history.last().unwrap();
                    moves.truncate(moves.len() - 2);
                    positions.truncate(positions.len() - 2);
                    last_move = None;
                    let side = if game.turn().is_white() {
                        "White"
                    } else {
                        "Black"
                    };
                    render(game.board(), &moves, &format!("Undo. {} to move:", side), last_move, flip);
                } else {
                    render(game.board(), &moves, "Nothing to undo.", last_move, flip);
                }
                continue;
            }
            "resign" => {
                let winner = if game.turn().is_white() {
                    "Black"
                } else {
                    "White"
                };
                render(
                    game.board(),
                    &moves,
                    &format!("{} wins by resignation! Press Enter to exit.", winner),
                    last_move,
                    flip,
                );
                let _ = stdin().read_line(&mut String::new());
                break;
            }
            "draw" => {
                render(
                    game.board(),
                    &moves,
                    "Draw by agreement! Press Enter to exit.",
                    last_move,
                    flip,
                );
                let _ = stdin().read_line(&mut String::new());
                break;
            }
            "help" | "h" => {
                render(
                    game.board(),
                    &moves,
                    "Commands: e4 (pawn), Nf3 (piece), O-O (castle), undo (u), resign, draw, help (h)",
                    last_move,
                    flip,
                );
                continue;
            }
            _ => {}
        }
        match San::from_str(&normalize_san(&input)) {
            Err(_) => {
                let hint = if input.trim().starts_with('b') {
                    " (use B for bishop, e.g. Bc3)"
                } else {
                    ""
                };
                render(
                    game.board(),
                    &moves,
                    &format!(
                        "Invalid move. Use e4 (pawn), Nf3 (piece), or O-O (castle):{}",
                        hint
                    ),
                    last_move,
                    flip,
                );
                continue;
            }
            Ok(san) => match san.to_move(&game) {
                Err(_) => {
                    render(game.board(), &moves, "Illegal move. Try again:", last_move, flip);
                    continue;
                }
                Ok(mov) => {
                    if let Err(e) = apply_move(
                        &mut game,
                        &mov,
                        &mut halfmove_clock,
                        &mut moves,
                        &mut positions,
                        &mut game_states,
                        &mut clock_history,
                    ) {
                        eprintln!("{}", e);
                        break;
                    }
                    last_move = Some((mov.from().unwrap(), mov.to()));

                    if let Some(outcome) = game.outcome() {
                        render(game.board(), &moves, &format!("Game over! {}", outcome), last_move, flip);
                        break;
                    }
                    if let Some(msg) = check_draw(&positions, halfmove_clock) {
                        render(game.board(), &moves, &msg, last_move, flip);
                        break;
                    }

                    render(game.board(), &moves, "Thinking...", last_move, flip);

                    match play_engine_move(&engine, &mut game, &mut halfmove_clock, &mut moves, &mut positions, &mut game_states, &mut clock_history) {
                        Ok(m) => {
                            last_move = Some((m.from().unwrap(), m.to()));
                        }
                        Err(e) => {
                            render(game.board(), &moves, &e, last_move, flip);
                            break;
                        }
                    }

                    if let Some(outcome) = game.outcome() {
                        render(game.board(), &moves, &format!("Game over! {}", outcome), last_move, flip);
                        break;
                    }
                    if let Some(msg) = check_draw(&positions, halfmove_clock) {
                        render(game.board(), &moves, &msg, last_move, flip);
                        break;
                    }

                    let side = if game.turn().is_white() {
                        "White"
                    } else {
                        "Black"
                    };
                    render(game.board(), &moves, &format!("{} to move:", side), last_move, flip);
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
        assert_eq!(piece_to_char(w), "\x1b[97m♙");
        let w = Piece { color: Color::White, role: Role::Knight };
        assert_eq!(piece_to_char(w), "\x1b[97m♘");
        let w = Piece { color: Color::White, role: Role::Bishop };
        assert_eq!(piece_to_char(w), "\x1b[97m♗");
        let w = Piece { color: Color::White, role: Role::Rook };
        assert_eq!(piece_to_char(w), "\x1b[97m♖");
        let w = Piece { color: Color::White, role: Role::Queen };
        assert_eq!(piece_to_char(w), "\x1b[97m♕");
        let w = Piece { color: Color::White, role: Role::King };
        assert_eq!(piece_to_char(w), "\x1b[97m♔");

        let b = Piece { color: Color::Black, role: Role::Pawn };
        assert_eq!(piece_to_char(b), "\x1b[31m♟");
        let b = Piece { color: Color::Black, role: Role::Knight };
        assert_eq!(piece_to_char(b), "\x1b[31m♞");
        let b = Piece { color: Color::Black, role: Role::Bishop };
        assert_eq!(piece_to_char(b), "\x1b[31m♝");
        let b = Piece { color: Color::Black, role: Role::Rook };
        assert_eq!(piece_to_char(b), "\x1b[31m♜");
        let b = Piece { color: Color::Black, role: Role::Queen };
        assert_eq!(piece_to_char(b), "\x1b[31m♛");
        let b = Piece { color: Color::Black, role: Role::King };
        assert_eq!(piece_to_char(b), "\x1b[31m♚");
    }

    #[test]
    fn test_check_draw_none_for_short_game() {
        let game = Chess::default();
        let positions = vec![board_fen(&game)];
        assert_eq!(check_draw(&positions, 0), None);
        assert_eq!(check_draw(&positions, 99), None);
    }

    #[test]
    fn test_check_draw_fifty_move_rule() {
        let game = Chess::default();
        let positions = vec![board_fen(&game)];
        assert_eq!(
            check_draw(&positions, 100),
            Some("Draw by 50-move rule".to_string())
        );
        assert_eq!(
            check_draw(&positions, 101),
            Some("Draw by 50-move rule".to_string())
        );
    }

    #[test]
    fn test_check_draw_threefold_repetition() {
        let positions = vec!["fen1".into(), "fen2".into(), "fen1".into(), "fen1".into()];
        assert_eq!(
            check_draw(&positions, 0),
            Some("Draw by threefold repetition".to_string())
        );
    }

    #[test]
    fn test_check_draw_threefold_not_reached() {
        let positions = vec!["fen1".into(), "fen2".into(), "fen1".into()];
        assert_eq!(check_draw(&positions, 0), None);
    }

    #[test]
    fn test_apply_move_invalid() {
        let mut game = Chess::default();
        let mut halfmove_clock = 0u32;
        let mut moves = vec![];
        let mut positions = vec![];
        let mut game_states = vec![];
        let mut clock_history = vec![];
        let mov = shakmaty::Move::Normal {
            role: Role::Pawn,
            from: shakmaty::Square::E2,
            to: shakmaty::Square::E5,
            capture: None,
            promotion: None,
        };
        assert!(apply_move(
            &mut game,
            &mov,
            &mut halfmove_clock,
            &mut moves,
            &mut positions,
            &mut game_states,
            &mut clock_history,
        )
        .is_err());
    }

    #[test]
    fn test_apply_move_valid() {
        let mut game = Chess::default();
        let mut halfmove_clock = 0u32;
        let mut moves = vec![];
        let mut positions = vec![];
        let mut game_states = vec![];
        let mut clock_history = vec![];
        let san = San::from_str("e4").unwrap();
        let mov = san.to_move(&game).unwrap();
        assert!(apply_move(
            &mut game,
            &mov,
            &mut halfmove_clock,
            &mut moves,
            &mut positions,
            &mut game_states,
            &mut clock_history,
        )
        .is_ok());
        assert_eq!(moves.len(), 1);
        assert_eq!(game_states.len(), 1);
        assert_eq!(clock_history.len(), 1);
        assert_eq!(halfmove_clock, 0);
    }

    #[test]
    fn test_apply_move_non_pawn_increments_clock() {
        let mut game = Chess::default();
        let m1 = San::from_str("e4").unwrap().to_move(&game).unwrap();
        game = game.play(&m1).unwrap();
        let m2 = San::from_str("e5").unwrap().to_move(&game).unwrap();
        game = game.play(&m2).unwrap();
        let mut halfmove_clock = 5u32;
        let mut moves = vec![];
        let mut positions = vec![];
        let mut game_states = vec![];
        let mut clock_history = vec![];
        let san = San::from_str("Nf3").unwrap();
        let mov = san.to_move(&game).unwrap();
        assert!(apply_move(
            &mut game,
            &mov,
            &mut halfmove_clock,
            &mut moves,
            &mut positions,
            &mut game_states,
            &mut clock_history,
        )
        .is_ok());
        assert_eq!(halfmove_clock, 6);
    }

    #[test]
    fn test_apply_move_pawn_resets_clock() {
        let mut game = Chess::default();
        let mut halfmove_clock = 10u32;
        let mut moves = vec![];
        let mut positions = vec![];
        let mut game_states = vec![];
        let mut clock_history = vec![];
        let san = San::from_str("e4").unwrap();
        let mov = san.to_move(&game).unwrap();
        assert!(apply_move(
            &mut game,
            &mov,
            &mut halfmove_clock,
            &mut moves,
            &mut positions,
            &mut game_states,
            &mut clock_history,
        )
        .is_ok());
        assert_eq!(halfmove_clock, 0);
    }

    #[test]
    fn test_apply_move_capture_resets_clock() {
        let mut game = Chess::default();
        let m1 = San::from_str("e4").unwrap().to_move(&game).unwrap();
        game = game.play(&m1).unwrap();
        let m2 = San::from_str("d5").unwrap().to_move(&game).unwrap();
        game = game.play(&m2).unwrap();
        let mut halfmove_clock = 10u32;
        let mut moves = vec![];
        let mut positions = vec![];
        let mut game_states = vec![];
        let mut clock_history = vec![];
        let san = San::from_str("exd5").unwrap();
        let mov = san.to_move(&game).unwrap();
        assert!(apply_move(
            &mut game,
            &mov,
            &mut halfmove_clock,
            &mut moves,
            &mut positions,
            &mut game_states,
            &mut clock_history,
        )
        .is_ok());
        assert_eq!(halfmove_clock, 0);
    }

    #[test]
    fn test_board_fen_initial() {
        let game = Chess::default();
        assert_eq!(
            board_fen(&game),
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
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
