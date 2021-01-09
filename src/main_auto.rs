// Copyright (C) 2020-2021 Andy Kurnia. All rights reserved.

use super::{alphabet, display, error, game_config, game_state, klv, kwg, movegen, prob, stats};
use rand::prelude::*;

struct WriteableRack<'a> {
    alphabet: &'a alphabet::Alphabet<'a>,
    rack: &'a [u8],
}

impl std::fmt::Display for WriteableRack<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for &tile in self.rack {
            write!(f, "{}", self.alphabet.from_rack(tile).unwrap())?;
        }
        Ok(())
    }
}

fn printable_rack<'a>(alphabet: &'a alphabet::Alphabet<'a>, rack: &'a [u8]) -> WriteableRack<'a> {
    WriteableRack {
        alphabet: &alphabet,
        rack: &rack,
    }
}

fn rack_score<'a>(alphabet: &'a alphabet::Alphabet<'a>, rack: &'a [u8]) -> i16 {
    rack.iter().map(|&t| alphabet.score(t) as i16).sum::<i16>()
}

// need more realistic numbers, and should differ by bot level
static LENGTH_IMPORTANCES: &[f32] = &[
    0.0, 0.0, 2.0, 1.5, 1.0, 0.75, 0.5, 1.0, 1.0, 0.5, 0.4, 0.3, 0.2, 0.1, 0.1, 0.1,
];

pub fn main() -> error::Returns<()> {
    let mut candidates = Vec::new();
    let kwg = kwg::Kwg::from_bytes_alloc(&std::fs::read("csw19.kwg")?);
    let klv = klv::Klv::from_bytes_alloc(&std::fs::read("leaves.klv")?);
    let game_config = &game_config::make_common_english_game_config();
    let _ = &game_config::make_super_english_game_config();
    let _ = &game_config::make_polish_game_config();
    let mut move_generator = movegen::KurniaMoveGenerator::new(game_config);
    let mut word_check_buf = Vec::new();

    let mut word_prob = prob::WordProbability::new(&game_config.alphabet());
    let max_prob_by_len = word_prob.get_max_probs_by_len(&kwg);
    println!("max prob: {:?}", max_prob_by_len);

    loop {
        let mut game_state = game_state::GameState::new(game_config);

        let mut zero_turns = 0;
        println!("\nplaying self");
        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();

        game_state.bag.shuffle(&mut rng);

        println!(
            "bag: {}",
            printable_rack(&game_state.game_config.alphabet(), &game_state.bag.0)
        );

        for player in game_state.players.iter_mut() {
            game_state.bag.replenish(
                &mut player.rack,
                game_state.game_config.rack_size() as usize,
            );
        }

        loop {
            display::print_board(
                &game_state.game_config.alphabet(),
                &game_state.game_config.board_layout(),
                &game_state.board_tiles,
            );
            for (i, player) in (1..).zip(game_state.players.iter()) {
                print!("player {}: {}, ", i, player.score);
            }
            println!("turn: player {}", game_state.turn + 1);

            let bot_level = if game_state.turn == 0 { 6 } else { 3 };
            let length_importances = LENGTH_IMPORTANCES; // should differ by bot level
            let mut tilt_factor = rng.gen_range(0.5 - bot_level as f64 * 0.1, 1.0);
            if tilt_factor < 0.0 {
                tilt_factor = 0.0;
            }
            let _ = tilt_factor;
            tilt_factor = 0.0; // let's just disable this
            println!("effective tilt factor for this turn: {}", tilt_factor);
            let mut leave_scale = (bot_level as f64 * 0.1 + (1.0 - tilt_factor)) as f32;
            if leave_scale > 1.0 {
                leave_scale = 1.0;
            }
            println!("effective leave scale for this turn: {}", leave_scale);
            let mut word_is_ok = |word: &[u8]| {
                if tilt_factor >= 1.0 {
                    return true;
                }
                let this_wp = word_prob.count_ways(word);
                let max_wp = max_prob_by_len[word.len()];
                let some_prob = 1.0 - (1.0 - (this_wp as f64 / max_wp as f64)).powi(2);
                let handwavy = length_importances[word.len()] as f64 * some_prob;
                if handwavy >= tilt_factor {
                    return true;
                }
                println!(
                    "Rejecting word {:?}, handwavy={} (this={} over max={}), tilt={}",
                    word, handwavy, this_wp, max_wp, tilt_factor
                );
                false
            };

            println!(
                "pool {:2}: {}",
                game_state.bag.0.len(),
                printable_rack(&game_state.game_config.alphabet(), &game_state.bag.0)
            );
            for (i, player) in (1..).zip(game_state.players.iter()) {
                println!(
                    "p{} rack: {}",
                    i,
                    printable_rack(&game_state.game_config.alphabet(), &player.rack)
                );
            }

            let board_snapshot = &movegen::BoardSnapshot {
                board_tiles: &game_state.board_tiles,
                game_config,
                kwg: &kwg,
                klv: &klv,
            };

            let validate_word_subset = |board_snapshot: &movegen::BoardSnapshot,
                                        down: bool,
                                        lane: i8,
                                        idx: i8,
                                        word: &[u8],
                                        _score: i16,
                                        _rack_tally: &[u8]| {
                let board_layout = board_snapshot.game_config.board_layout();
                let dim = board_layout.dim();
                let strider = if down {
                    dim.down(lane)
                } else {
                    dim.across(lane)
                };
                word_check_buf.clear();
                for (i, &tile) in (idx..).zip(word.iter()) {
                    let placed_tile = if tile != 0 {
                        tile
                    } else {
                        board_snapshot.board_tiles[strider.at(i)]
                    };
                    word_check_buf.push(placed_tile & 0x7f);
                }
                if !word_is_ok(&word_check_buf) {
                    return false;
                }
                for (i, &tile) in (idx..).zip(word.iter()) {
                    if tile != 0 {
                        let perpendicular_strider = if down { dim.across(i) } else { dim.down(i) };
                        let mut j = lane;
                        while j > 0
                            && board_snapshot.board_tiles[perpendicular_strider.at(j - 1)] != 0
                        {
                            j -= 1;
                        }
                        let perpendicular_strider_len = perpendicular_strider.len();
                        if j == lane
                            && if j + 1 < perpendicular_strider_len {
                                board_snapshot.board_tiles[perpendicular_strider.at(j + 1)] == 0
                            } else {
                                true
                            }
                        {
                            // no perpendicular tile
                            continue;
                        }
                        word_check_buf.clear();
                        for j in j..perpendicular_strider_len {
                            let placed_tile = if j == lane {
                                tile
                            } else {
                                board_snapshot.board_tiles[perpendicular_strider.at(j)]
                            };
                            if placed_tile == 0 {
                                break;
                            }
                            word_check_buf.push(placed_tile & 0x7f);
                        }
                        if !word_is_ok(&word_check_buf) {
                            return false;
                        }
                    }
                }
                true
            };
            let _ = validate_word_subset;
            let validate_word_subset =
                |_board_snapshot: &movegen::BoardSnapshot,
                 _down: bool,
                 _lane: i8,
                 _idx: i8,
                 _word: &[u8],
                 _score: i16,
                 _rack_tally: &[u8]| { true };
            let adjust_leave_value = |leave_value: f32| leave_scale * leave_value;
            move_generator.gen_moves_alloc(
                board_snapshot,
                &game_state.current_player().rack,
                100,
                |down: bool, lane: i8, idx: i8, word: &[u8], score: i16, rack_tally: &[u8]| {
                    validate_word_subset(&board_snapshot, down, lane, idx, word, score, rack_tally)
                },
                adjust_leave_value,
            );
            let plays = &mut move_generator.plays;

            println!("found {} moves", plays.len());
            for play in plays.iter().take(10) {
                println!("{} {}", play.equity, play.play.fmt(board_snapshot));
            }

            println!("let's sim them");
            struct Candidate {
                play: movegen::Play,
                equity: f32,
                stats: stats::Stats,
            }
            candidates.clear();
            candidates.reserve(plays.len());
            for play in plays.drain(..) {
                candidates.push(Candidate {
                    play: play.play,
                    equity: play.equity,
                    stats: stats::Stats::new(),
                });
            }
            {
                let mut simmer_rng = rand_chacha::ChaCha20Rng::from_entropy();
                let mut simmer_move_generator = movegen::KurniaMoveGenerator::new(game_config);
                let mut simmer_initial_game_state = game_state.clone(); // will be overwritten
                let initial_spread = if simmer_initial_game_state.players.len() < 2 {
                    0
                } else {
                    simmer_initial_game_state.current_player().score
                        - (0..)
                            .zip(simmer_initial_game_state.players.iter())
                            .filter(|&(i, _)| i != simmer_initial_game_state.turn)
                            .map(|(_, player)| player.score)
                            .max()
                            .unwrap()
                };
                let mut last_seen_leave_values =
                    vec![0.0f32; simmer_initial_game_state.players.len()];
                let mut simmer_game_state = simmer_initial_game_state.clone(); // will be overwritten
                let mut simmer_rack_tally =
                    vec![0u8; game_config.alphabet().len() as usize].into_boxed_slice(); // will be overwritten
                let num_sim_iters = 1000;
                let mut when_to_prune = 16;
                let num_sim_plies = 2;
                let num_tiles_that_matter = num_sim_plies * game_config.rack_size() as usize;
                let t0 = std::time::Instant::now();
                for sim_iter in 0..num_sim_iters {
                    if (sim_iter + 1) % 100 == 0 {
                        println!(
                            "{} iters in {:?}, {} moves",
                            sim_iter,
                            t0.elapsed(),
                            candidates.len()
                        );
                    }
                    loop {
                        simmer_initial_game_state.next_turn();
                        if simmer_initial_game_state.turn == game_state.turn {
                            break;
                        }
                        let player = &mut simmer_initial_game_state.players
                            [simmer_initial_game_state.turn as usize];
                        simmer_initial_game_state
                            .bag
                            .put_back(&mut rng, &player.rack);
                        player.rack.clear();
                    }
                    let possible_to_play_out =
                        simmer_initial_game_state.bag.0.len() <= num_tiles_that_matter;
                    simmer_initial_game_state
                        .bag
                        .shuffle_n(&mut simmer_rng, num_tiles_that_matter);
                    last_seen_leave_values.iter_mut().for_each(|m| *m = 0.0);
                    loop {
                        simmer_initial_game_state.next_turn();
                        if simmer_initial_game_state.turn == game_state.turn {
                            break;
                        }
                        let player = &mut simmer_initial_game_state.players
                            [simmer_initial_game_state.turn as usize];
                        simmer_initial_game_state.bag.replenish(
                            &mut player.rack,
                            game_state.players[simmer_initial_game_state.turn as usize]
                                .rack
                                .len(),
                        );
                    }
                    for candidate in candidates.iter_mut() {
                        simmer_game_state.clone_from(&simmer_initial_game_state);
                        let mut played_out = false;
                        for ply in 0..=num_sim_plies {
                            let simmer_board_snapshot = &movegen::BoardSnapshot {
                                board_tiles: &simmer_game_state.board_tiles,
                                game_config,
                                kwg: &kwg,
                                klv: &klv,
                            };
                            let next_play = if ply == 0 {
                                &candidate.play
                            } else {
                                simmer_move_generator.gen_moves_alloc(
                                    simmer_board_snapshot,
                                    &simmer_game_state.current_player().rack,
                                    1,
                                    |down: bool,
                                     lane: i8,
                                     idx: i8,
                                     word: &[u8],
                                     score: i16,
                                     rack_tally: &[u8]| {
                                        validate_word_subset(
                                            &simmer_board_snapshot,
                                            down,
                                            lane,
                                            idx,
                                            word,
                                            score,
                                            rack_tally,
                                        )
                                    },
                                    adjust_leave_value,
                                );
                                &simmer_move_generator.plays[0].play
                            };
                            simmer_rack_tally.iter_mut().for_each(|m| *m = 0);
                            simmer_game_state
                                .current_player()
                                .rack
                                .iter()
                                .for_each(|&tile| simmer_rack_tally[tile as usize] += 1);
                            match &next_play {
                                movegen::Play::Exchange { tiles } => {
                                    tiles
                                        .iter()
                                        .for_each(|&tile| simmer_rack_tally[tile as usize] -= 1);
                                }
                                movegen::Play::Place { word, .. } => {
                                    word.iter().for_each(|&tile| {
                                        if tile & 0x80 != 0 {
                                            simmer_rack_tally[0] -= 1;
                                        } else if tile != 0 {
                                            simmer_rack_tally[tile as usize] -= 1;
                                        }
                                    });
                                }
                            };
                            let leave_value = klv.leave_value_from_tally(&simmer_rack_tally);
                            last_seen_leave_values[simmer_game_state.turn as usize] = leave_value;
                            simmer_game_state.play(&mut simmer_rng, &next_play)?;
                            if simmer_game_state.current_player().rack.is_empty() {
                                played_out = true;
                                break;
                            }
                            simmer_game_state.next_turn();
                        }
                        if played_out {
                            // not handling the too-many-zeros case
                            if simmer_game_state.players.len() == 2 {
                                simmer_game_state.players[simmer_game_state.turn as usize].score +=
                                    2 * rack_score(
                                        &game_config.alphabet(),
                                        &simmer_game_state.players
                                            [(1 - simmer_game_state.turn) as usize]
                                            .rack,
                                    );
                            } else {
                                let mut earned = 0;
                                for mut player in simmer_game_state.players.iter_mut() {
                                    let this_rack =
                                        rack_score(&game_config.alphabet(), &player.rack);
                                    player.score -= this_rack;
                                    earned += this_rack;
                                }
                                simmer_game_state.players[simmer_game_state.turn as usize].score +=
                                    earned;
                            }
                            last_seen_leave_values.iter_mut().for_each(|m| *m = 0.0);
                        }
                        let mut best_opponent = simmer_initial_game_state.turn;
                        let mut best_opponent_equity = f32::NEG_INFINITY;
                        for (i, player) in (0..).zip(simmer_game_state.players.iter()) {
                            if i != simmer_initial_game_state.turn {
                                let opponent_equity =
                                    player.score as f32 + last_seen_leave_values[i as usize];
                                if opponent_equity > best_opponent_equity {
                                    best_opponent = i;
                                    best_opponent_equity = opponent_equity;
                                }
                            }
                        }
                        let mut this_equity = simmer_game_state.players
                            [simmer_initial_game_state.turn as usize]
                            .score as f32
                            + last_seen_leave_values[simmer_initial_game_state.turn as usize];
                        if best_opponent != simmer_initial_game_state.turn {
                            this_equity -= best_opponent_equity;
                        }
                        let sim_spread = this_equity - initial_spread as f32;
                        let win_probability;
                        if played_out {
                            win_probability = if sim_spread > 0.0 {
                                1.0
                            } else if sim_spread < 0.0 {
                                0.0
                            } else {
                                0.5
                            };
                        } else {
                            // handwavily: assume spread of +/- (30 + num_unseen_tiles) should be 90%/10% (-Andy Kurnia)
                            let num_unseen_tiles = simmer_game_state.bag.0.len()
                                + simmer_game_state
                                    .players
                                    .iter()
                                    .map(|player| player.rack.len())
                                    .sum::<usize>();
                            // this could be precomputed for every possible num_unseen_tiles (1 to 93)
                            let exp_width =
                                -(30.0 + num_unseen_tiles as f64) / ((1.0 / 0.9 - 1.0) as f64).ln();
                            win_probability =
                                1.0 / (1.0 + (-(sim_spread as f64) / exp_width).exp());
                        }
                        candidate.stats.update(
                            sim_spread as f64
                                + win_probability
                                    * if possible_to_play_out { 1000.0 } else { 10.0 },
                        );
                    }
                    if (sim_iter + 1) == when_to_prune {
                        when_to_prune <<= 1;
                        // confidence interval = mean +/- Z * stddev / sqrt(samples)
                        // Z = 1.96 for 95% CI
                        // first find the top candidate based on max range
                        let ci_z = 1.96;
                        // assume all surviving candidate moves have been simmed for the same number of samples
                        let ci_z_over_sqrt_n = ci_z / ((sim_iter + 1) as f64).sqrt();
                        let top_candidate = candidates
                            .iter()
                            .max_by(|a, b| {
                                (a.stats.mean() + ci_z_over_sqrt_n * a.stats.standard_deviation())
                                    .partial_cmp(
                                        &(b.stats.mean()
                                            + ci_z_over_sqrt_n * b.stats.standard_deviation()),
                                    )
                                    .unwrap()
                            })
                            .unwrap();
                        let low_bar = top_candidate.stats.mean()
                            - ci_z_over_sqrt_n * top_candidate.stats.standard_deviation();
                        println!(
                            "top play after {} iters: {} {}: {} samples, {} mean, {} stddev; low bar: {}",
                            sim_iter+1,
                            top_candidate.equity,
                            top_candidate.play.fmt(board_snapshot),
                            top_candidate.stats.count(),
                            top_candidate.stats.mean(),
                            top_candidate.stats.standard_deviation(),
                            low_bar,
                        );
                        // remove candidates that cannot catch up
                        candidates.retain(|candidate| {
                            (candidate.stats.mean()
                                + ci_z_over_sqrt_n * candidate.stats.standard_deviation())
                                >= low_bar
                        });
                        println!(
                            "{} iters in {:?}, {} moves",
                            sim_iter + 1,
                            t0.elapsed(),
                            candidates.len()
                        );
                        if candidates.len() < 2 {
                            println!("only one candidate move left, no need to continue");
                            break;
                        }
                    }
                }
            }
            candidates.sort_unstable_by(|a, b| {
                b.stats
                    .mean()
                    .partial_cmp(&a.stats.mean())
                    .unwrap()
                    .then_with(|| b.equity.partial_cmp(&a.equity).unwrap())
            });
            for candidate in candidates.iter() {
                println!(
                    "{} {}: {} samples, {} mean, {} stddev",
                    candidate.equity,
                    candidate.play.fmt(board_snapshot),
                    candidate.stats.count(),
                    candidate.stats.mean(),
                    candidate.stats.standard_deviation()
                );
            }

            let play = &candidates[0].play; // assume at least there's always Pass
            println!("making top move: {}", play.fmt(board_snapshot));

            game_state.play(&mut rng, play)?;

            zero_turns += 1;
            if match play {
                movegen::Play::Exchange { .. } => 0,
                movegen::Play::Place { score, .. } => *score,
            } != 0
            {
                zero_turns = 0;
            }

            if game_state.current_player().rack.is_empty() {
                display::print_board(
                    &game_state.game_config.alphabet(),
                    &game_state.game_config.board_layout(),
                    &game_state.board_tiles,
                );
                for (i, player) in (1..).zip(game_state.players.iter()) {
                    print!("player {}: {}, ", i, player.score);
                }
                println!(
                    "player {} went out (scores are before leftovers)",
                    game_state.turn + 1
                );
                if game_state.players.len() == 2 {
                    game_state.players[game_state.turn as usize].score += 2 * rack_score(
                        &game_state.game_config.alphabet(),
                        &game_state.players[(1 - game_state.turn) as usize].rack,
                    );
                } else {
                    let mut earned = 0;
                    for mut player in game_state.players.iter_mut() {
                        let this_rack =
                            rack_score(&game_state.game_config.alphabet(), &player.rack);
                        player.score -= this_rack;
                        earned += this_rack;
                    }
                    game_state.players[game_state.turn as usize].score += earned;
                }
                break;
            }

            if zero_turns >= game_state.players.len() * 3 {
                display::print_board(
                    &game_state.game_config.alphabet(),
                    &game_state.game_config.board_layout(),
                    &game_state.board_tiles,
                );
                for (i, player) in (1..).zip(game_state.players.iter()) {
                    print!("player {}: {}, ", i, player.score);
                }
                println!(
                    "player {} ended game by making yet another zero score",
                    game_state.turn + 1
                );
                for mut player in game_state.players.iter_mut() {
                    player.score -= rack_score(&game_state.game_config.alphabet(), &player.rack);
                }
                break;
            }

            game_state.next_turn();
        }

        for (i, player) in (1..).zip(game_state.players.iter()) {
            print!("player {}: {}, ", i, player.score);
        }
        println!("final scores");
    } // temp loop

    //Ok(())
}
