#![feature(drain_filter)]
#![feature(in_band_lifetimes)]

use std::cmp::{Ordering};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::{Range, RangeInclusive};
use std::fmt::{Display, Formatter, Error};
use std::io;
use std::io::prelude::*;

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}

#[derive(Debug)]
pub enum SudokuError {
    NoRemainingValues,
}

pub enum SudokuResult{
    OneCandidate(SudokuSquare),
    MultipleCandidates(Vec<SudokuSquare>),
}

pub trait RangeInterval {
    fn interval(&self) -> (usize, usize);
}

impl RangeInterval for Range<usize>{
    fn interval(&self) -> (usize, usize){
        let x = self.clone();
        (self.start, x.last().unwrap())
    }
}

impl RangeInterval for RangeInclusive<usize>{
    fn interval(&self) -> (usize, usize){
        (*self.start(), *self.end())
    }
}

fn create_square_vec(rows: Box<dyn RangeInterval>, cols: Box<dyn RangeInterval>) -> Vec<SudokuSquare>{
    let mut v = Vec::new();

    for i in rows.interval().0..=rows.interval().1 {
        for j in cols.interval().0..=cols.interval().1 {
            v.push(SudokuSquare(i, j));
        }
    }
    v
}

fn create_unitlist(r: Range<usize>, c: Range<usize>) -> Vec<Vec<SudokuSquare>> {

    let max_rows = r.end;
    let max_cols = c.end;

    let mut v = Vec::with_capacity(max_rows * 3);

    for row in 0..max_rows {
        v.push(create_square_vec(Box::new(row..=row),
                                 Box::new(0..max_cols)));
    }

    for col in 0..max_cols {
        v.push( create_square_vec( Box::new(0..max_rows),
                                   Box::new(col..=col)));
    }

    let matrices = vec![(0..=2), (3..=5), (6..=8)];

    for m in 0..matrices.len(){
        for n in 0..matrices.len(){
            v.push(create_square_vec(Box::new(matrices[m].clone()),
                                     Box::new(matrices[n].clone())));
        }
    }
    // Time to return
    v
}

fn create_unit_dictionary(unitlist: &Vec<Vec<SudokuSquare>>) -> HashMap<SudokuSquare, SudokuUnit> {
    let mut unitmap: HashMap<SudokuSquare, SudokuUnit> = HashMap::new();
    for v in unitlist{
        for &ve in v {
            match unitmap.contains_key(&ve){
                true => {
                    let value = unitmap.get_mut(&ve).unwrap();
                    value.unitvec.push(&v);
                },
                false => {
                    unitmap.insert(ve, SudokuUnit{unitvec: vec![v]});
                },
            }
        }
    }
    unitmap
}

fn create_peers_dictionary(unit_dict: &HashMap<SudokuSquare, SudokuUnit>)
                           -> HashMap<SudokuSquare, HashSet<SudokuSquare>> {
    let mut peers: HashMap<SudokuSquare, HashSet<SudokuSquare>> = HashMap::new();

    for unit in unit_dict.iter(){
        let mut set = HashSet::new();
        for &v in &unit.1.unitvec {
            set.extend( v.clone().drain_filter(|&mut x| x != *unit.0) );
        }
        peers.insert(unit.0.clone(), set);
    }
    peers
}


#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct SudokuSquare(usize, usize);

impl PartialOrd<Self> for SudokuSquare {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SudokuSquare {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0 == other.0 {
            return self.1.cmp(&other.1);
        }else{
            return self.0.cmp( &other.0);
        }
    }
}

#[derive(Debug, Clone)]
struct SudokuUnit<'a>{
    unitvec: Vec<&'a Vec<SudokuSquare>>,
}

struct GameSetup<'a>{
    squares:  &'a Vec<Vec<SudokuSquare>>,
    units:  &'a HashMap<SudokuSquare, SudokuUnit<'a>>,
    peers:  &'a HashMap<SudokuSquare, HashSet<SudokuSquare>>,
    sorted_squares: Vec<SudokuSquare>,
}

impl GameSetup<'a>{
    fn new(squares: &'a Vec<Vec<SudokuSquare>>,
           units: &'a HashMap<SudokuSquare, SudokuUnit>,
           peers: &'a HashMap<SudokuSquare, HashSet<SudokuSquare>>) -> Self {

        let mut _sorted = vec![];
        for (k, _v) in units.iter() {
            _sorted.push(k.clone());
        }

        _sorted.sort();

        Self {
            squares,
            units,
            peers,
            sorted_squares: _sorted,
        }
    }
}

#[derive(Clone)]
struct Game<'a> {
    game_setup: &'a GameSetup<'a>,
    stats: HashMap<SudokuSquare, String>,
}

impl Game<'a> {
    pub fn new(game_setup: &'a GameSetup) -> Self {
        Self {
            game_setup,
            stats: HashMap::new(),
        }
    }

    pub fn init_game_with_values(&mut self, values: &str) {
        let value_chars = values.chars();
        // assert_eq!(value_chars.count(), self.game_setup.squares.len());

        let mut square_set = BTreeSet::new();

        for unit in &self.game_setup.sorted_squares {
            square_set.insert(unit.clone() );
            self.stats.insert(unit.clone(), String::from("123456789"));
        }
        // Throw in a small sanity check.
        // let char_length = value_chars.by_ref().count();
        // assert_eq!(square_set.len(), char_length);

        let mut it = value_chars.into_iter();
        for set_element in square_set {
            let c = it.next().unwrap();
            if matches!(c, '1'..='9') {
                self.assign(&set_element, c);
            }
        }
    }

    pub fn assign(&mut self, square: &SudokuSquare, c: char) -> Result<(), SudokuError>{
        let unit_stored = self.stats.get_key_value(square).unwrap();
        let mut values = unit_stored.1.clone();
        values = values.replace(c, "");
        for c in values.chars(){
            self.eliminate(&square, c)?;
        }
        Ok(())
    }

    fn eliminate(&mut self, square: &SudokuSquare, c: char) -> Result<(), SudokuError>{
        let unit_stored = self.stats.get_key_value(square).unwrap();
        if !unit_stored.1.contains(c) {
            return Ok(());
        } else{
            let new_value = unit_stored.1.replace(c, "");
            if new_value.len() == 0 {
                return Err(SudokuError::NoRemainingValues);
            }
            self.stats.insert(square.clone(), new_value);
        }

        let unit_stored = self.stats.get_key_value(square).unwrap();
        if unit_stored.1.len() == 1 {
            let last_char_remaining = unit_stored.1.chars().next().unwrap();
            let peers = self.game_setup.peers.get(&square).unwrap();
            for p in peers.iter() {
                self.eliminate(p, last_char_remaining)?;
            }
        }

        for unit in self.game_setup.units.get(&square) {
            for solidary_unit in &unit.unitvec {
                match self.count_places_for_value(solidary_unit, c){
                    Ok(SudokuResult::MultipleCandidates(_)) => continue,
                    Ok(SudokuResult::OneCandidate(candidate)) =>
                        return self.assign(&candidate, c),
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(())
    }

    fn count_places_for_value(&self, square_vec: &Vec<SudokuSquare>, c: char ) ->
    Result<SudokuResult, SudokuError>{
        let mut candidates = vec![];
        for s in square_vec {
            match self.stats.get_key_value(s).unwrap().1.contains(c){
                true => candidates.push(s.clone()),
                _ => (),
            }
        }

        return match candidates.len() {
            0 => Err(SudokuError::NoRemainingValues),
            1 => Ok(SudokuResult::OneCandidate(candidates[0])),
            _ => Ok(SudokuResult::MultipleCandidates(candidates)),
        }
    }

    pub fn search(&self) {
        let solved_status = self.is_solved();
        match solved_status {
            Err(_error) => (),
            Ok((true, _)) => {
                println!("{}", self);
                pause()
            },
            Ok((false, candidate)) => {
                let candidate = self.stats.get_key_value(&candidate).unwrap();
                for c in candidate.1.chars(){
                    let mut game_branch = self.clone();
                    let assign_result = game_branch.assign(candidate.0, c);
                    if assign_result.is_ok() {
                        game_branch.search();
                    }
                }
            },
        }
    }

    fn is_solved(&self) -> Result<(bool, SudokuSquare), SudokuError> {

        let min_candidates = self.stats.iter()
            .fold((0, 9, SudokuSquare(0,0)), |acc, square|
                {
                    let len = square.1.len();
                    match len {
                        0 => (acc.0, 0, square.0.clone()),
                        1 => (acc.0 + 1, acc.1, acc.2),
                        _ if &len < &acc.1 => (acc.0, *&len, square.0.clone()),
                        _ => acc,
                    }
                });

        if min_candidates.1 == 0 {
            return Err(SudokuError::NoRemainingValues);
        }

        return if min_candidates.0 == self.stats.keys().len() {
            Ok((true, min_candidates.2))
        } else {
            Ok((false, min_candidates.2))
        }

    }
}

impl Display for Game<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut square_count = 0;
        let mut output = String::new();
        for square in &self.game_setup.sorted_squares {
            output.push_str(
                &format!("{number:>width$} ", number=self.stats.get(square).unwrap(), width=6));
            square_count += 1;
            if square_count % 9 == 0 { output.push('\n'); }
        }
        write!(f, "{}", output)
    }
}

impl Display for SudokuSquare {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let row =
            match self.0 {
                0..=8 => (('A' as u8) + (self.0 as u8)) as char,
                _ => panic!("Invalid Row Index"),
            };

        let col =
            match self.1 {
                0..=8 => self.1 + 1,
                _ => panic!("Invalid Column Index"),
            };

        write!(f, "{}{}", row, col)
    }
}

fn main() {

    let squares = create_unitlist(0..9, 0..9);
    let units = create_unit_dictionary(&squares);
    let peers = create_peers_dictionary(&units);

    let gs = GameSetup::new(&squares, &units, &peers);

    let mut game = Game::new(&gs);
    let puzzle = std::fs::read_to_string("sudoku.txt").unwrap();
    game.init_game_with_values(&puzzle);

    // println!("{}", game);

    game.search();

}
