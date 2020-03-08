extern crate base64;
extern crate clap;

use std::collections::BTreeSet;
use std::collections::BTreeMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(PartialEq)]
enum BoardArea {
  ROW,
  COL,
  REGION,
  ALL
}

/// Unflattens a list into an uncompressed game board
///
/// The uncompressed game board is an 81-element Vec
/// representing the 9x9 board, indexed right-to-left,
/// then top-down. An unassigned cell holds value 0.
/// An assigned cell holds the assigned value.
fn unflatten(list: &[u8]) -> Vec<u8> {
  let mut board = vec![0u8; 81];
  let mut cur: u8 = 0;
  let mut remaining: u8 = 0;
  enum State {
    READY,
    SIZE,
    READING
  };
  let mut state = State::READY;
  for v in list.iter() {
    match state {
      State::READY => {
        cur = *v;
        state = State::SIZE;
      },
      State::SIZE => {
        remaining = *v;
        state = State::READING;
      },
      State::READING => {
        board[*v as usize] = cur;
        remaining -= 1;
        if remaining == 0 {
          state = State::READY;
        }
      }
    }
  }
  board
}

/// Flattens a game board into a compressed list
///
/// The compressed list is a variable size Vec containing:
/// - The value n in play on the game board
/// - The number k of occurrences of value n
/// - The k indices where n occurs in the game board
/// - (repeat)
/// The values and indices are encoded in ascending order.
fn flatten(board: &[u8]) -> Vec<u8> {
  let mut list: Vec<u8> = Vec::new();
  let mut occurrences: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
  for i in 0u8..81 {
    let id: usize = i as usize;
    if board[id] != 0u8 {
      occurrences.entry(board[id]).or_default().push(i);
    }
  }
  for (n, indices) in &occurrences {
    list.push(*n);
    list.push(indices.len() as u8);
    for i in indices.iter() {
      list.push(*i);
    }
  }
  list
}

/// Prints an unflattened game board
fn print_board(board: &[u8]) {
  for row in 0..9 {
    for col in 0..9 {
      print!("{:3}", &board[id(row, col)]);
    }
    println!();
  }
}

/// Convert index from 2D to 1D
fn id(row: usize, col: usize) -> usize {
  9 * row + col
}

/// Given a 1D index, return the index of the first element in that row
fn get_row_start(i: usize) -> usize {
  (i / 9) * 9
}

/// Given a 1D index, return the index of the first element in that column
fn get_col_start(i: usize) -> usize {
  i % 9
}

/// Given a 1D index, return the index of the top-left element in that region
fn get_region_start(i: usize) -> usize {
  // {0, 27, 54}   + {0, 3, 6}
  ((i / 27) * 27) + (((i % 9) / 3) * 3)
}

/// Return a static, unmutable universe set (values 1 to 9)
fn get_universe() -> BTreeSet<u8> {
  // TODO: make this static?
  let universe: BTreeSet<u8> = (1u8..10u8).into_iter().collect();
  universe
}

/// Return a set of used values in the scope of the given cell
///
/// Checks the cell's row, column, and region
fn get_used(board: &[u8], i: usize, area: BoardArea) -> BTreeSet<u8> {
  let mut used: BTreeSet<u8> = BTreeSet::new();
  // Accumulate along row
  if area == BoardArea::ROW || area == BoardArea::ALL {
    let row_start = get_row_start(i);
    for j in row_start .. row_start + 9 {
      let value = board[j];
      if value != 0u8 {
        used.insert(value);
      }
    }
  }
  // Accumulate along column
  if area == BoardArea::COL || area == BoardArea::ALL {
    let col_start = get_col_start(i);
    for j in 0..9 {
      let value = board[9 * j + col_start];
      if value != 0u8 {
        used.insert(value);
      }
    }
  }
  // Accumulate in region
  if area == BoardArea::REGION || area == BoardArea::ALL {
    let region_start = get_region_start(i);
    for j in 0..3 {
      for k in 0..3 {
        let value = board[9 * j + region_start + k];
        if value != 0u8 {
          used.insert(value);
        }
      }
    }
  }
  used
}

/// Return a set of missing values in a given region
///
/// `area` can be `ROW`, `COL`, or `REGION`
/// `start` must be a valid starting index of the corresponding area
fn get_missing(board: &[u8], area: BoardArea, start: usize) -> BTreeSet<u8> {
  let used: BTreeSet<u8> = get_used(&board, start, area);
  get_universe().difference(&used).cloned().collect()
}

/// Assign values to unassigned cells in the board
///
/// Multiple rounds of solve may need to be called to solve the entire puzzle
/// Returns the number of assignments made in this round
fn solve(board: &mut [u8], verbose: bool) -> usize {
  let mut assigned: usize = 0;

  // Find used/free values for all cells
  for row in 0..9 {
    for col in 0..9 {
      let used = get_used(&board, id(row, col), BoardArea::ALL);
      let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
      if verbose {
        println!("At scope of ({},{}) [{}], used: {:?}, free: {:?}", row, col, id(row, col), used, free);
      }
      if board[id(row, col)] == 0u8 && free.len() == 1 {
        board[id(row, col)] = *free.iter().next().unwrap();
        assigned += 1;
        if verbose {
          println!("Assign [{}] to {}", id(row, col), board[id(row, col)]);
        }
      }
    }
  }

  if verbose {
    print_board(&board);
  }

  // Cross-reference missing values in board areas with free values in their cells

  // Row
  for row in 0..9 {
    let missing = get_missing(&board, BoardArea::ROW, id(row, 0));
    if verbose {
      println!("At row {}, missing: {:?}", row, missing);
    }
    // Go through all columns and record positions that can fulfill the missing value
    let mut candidates: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for col in 0..9 {
      if board[id(row, col)] == 0u8 { // unassigned cells only
        let used = get_used(&board, id(row, col), BoardArea::ALL);
        let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
        for value in &free {
          if missing.contains(&value) {
            candidates.entry(*value).or_default().push(id(row, col));
          }
        }
      }
    }
    // If any missing value can only be fulfilled by one position, assign it
    for (value, positions) in &candidates {
      if verbose && positions.len() > 0 {
        println!("Value {} can be fulfilled by positions: {:?}", value, positions);
      }
      if positions.len() == 1 {
        board[positions[0]] = *value;
        assigned += 1;
        if verbose {
          println!("Assign [{}] to {}", positions[0], *value);
        }
      }
    }
  }

  if verbose {
    print_board(&board);
  }

  // Column
  for col in 0..9 {
    let missing = get_missing(&board, BoardArea::COL, id(0, col));
    if verbose {
      println!("At column {}, missing: {:?}", col, missing);
    }
    // Go through all rows and record positions that can fulfill the missing value
    let mut candidates: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for row in 0..9 {
      if board[id(row, col)] == 0u8 { // unassigned cells only
        let used = get_used(&board, id(row, col), BoardArea::ALL);
        let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
        for value in &free {
          if missing.contains(&value) {
            candidates.entry(*value).or_default().push(id(row, col));
          }
        }
      }
    }
    // If any missing value can only be fulfilled by one position, assign it
    for (value, positions) in &candidates {
      if verbose && positions.len() > 0 {
        println!("Value {} can be fulfilled by positions: {:?}", value, positions);
      }
      if positions.len() == 1 {
        board[positions[0]] = *value;
        assigned += 1;
        if verbose {
          println!("Assign [{}] to {}", positions[0], *value);
        }
      }
    }
  }

  if verbose {
    print_board(&board);
  }

  // Region
  for start in vec![0, 3, 6, 27, 30, 33, 54, 57, 60] {
    let missing = get_missing(&board, BoardArea::REGION, start);
    if verbose {
      println!("At region {}, missing: {:?}", start, missing);
    }
    // Go through all cells of the region and record positions that can fulfill the missing value
    let mut candidates: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for row in 0..3 {
      for col in 0..3 {
        let pos = start + 9 * row + col;
        if board[pos] == 0u8 { // unassigned cells only
          let used = get_used(&board, pos, BoardArea::ALL);
          let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
          for value in &free {
            if missing.contains(&value) {
              candidates.entry(*value).or_default().push(pos);
            }
          }
        }
      }
    }
    // If any missing value can only be fulfilled by one position, assign it
    for (value, positions) in &candidates {
      if verbose && positions.len() > 0 {
        println!("Value {} can be fulfilled by positions: {:?}", value, positions);
      }
      if positions.len() == 1 {
        board[positions[0]] = *value;
        assigned += 1;
        if verbose {
          println!("Assign [{}] to {}", positions[0], *value);
        }
      }
    }
  }

  if verbose {
    println!("Made {} assignments", assigned);
    print_board(&board);
  }
  
  assigned
}

/// Returns true if the puzzle is solved
fn is_solved(board: &[u8]) -> bool {
  for value in board.iter() {
    if *value == 0u8 {
      return false;
    }
  }
  true
}

fn main() {
  let args = clap::App::new("sudoku")
    .arg(clap::Arg::with_name("seed")
      .short("-s")
      .long("--seed")
      .value_name("SEED")
      .help("base64 board state")
      .takes_value(true))
    .arg(clap::Arg::with_name("list")
      .short("-l")
      .long("--list")
      .value_name("LIST")
      .help("flattened list-based board state")
      .multiple(true)
      .takes_value(true))
    .arg(clap::Arg::with_name("board")
      .short("-b")
      .long("--board")
      .value_name("BOARD")
      .help("full list-based board state")
      .multiple(true)
      .takes_value(true))
    .arg(clap::Arg::with_name("verbose")
      .short("-v")
      .long("--verbose")
      .help("show solver steps")
      .takes_value(false))
    .group(clap::ArgGroup::with_name("input")
      .args(&["seed", "list", "board"])
      .required(true)
      .multiple(false))
    .get_matches();

  let verbose: bool = args.is_present("verbose");

  // Generate the seed, flattened list, and unflattened board
  let seed: String;
  let list: Vec<u8>;
  let mut board: Vec<u8>;

  if args.is_present("seed") {
    seed = args.value_of("seed").unwrap().to_string();
    list = base64::decode(&seed).unwrap();
    board = unflatten(&list);
  }
  else if args.is_present("list") {
    list = args.values_of("list").unwrap().collect::<Vec<_>>()
      .iter().map(|x| x.parse::<u8>().unwrap()).collect::<Vec<u8>>();
    seed = base64::encode(&list).to_string();
    board = unflatten(&list);
  }
  else { //if args.is_present("board") {
    board = args.values_of("board").unwrap().collect::<Vec<_>>()
      .iter().map(|x| x.parse::<u8>().unwrap()).collect::<Vec<u8>>();
    assert!(board.len() == 81);
    list = flatten(&board);
    seed = base64::encode(&list).to_string();
  }

  if ! args.is_present("seed") {
    println!("Game seed is {}", seed);
  }

  if verbose {
    println!("Printing board indices");
    for row in 0..9 {
      for col in 0..9 {
        print!("{:3}", id(row, col));
      }
      println!();
    }
    println!("---");
  }

  // Print the initial board state
  print_board(&board);

  // Testing: rebuild the list and seed
  if verbose {
    let rebuilt_list = flatten(&board);
    let rebuilt_seed = base64::encode(&rebuilt_list).to_string();
    if rebuilt_seed != seed {
      println!("Canonical form of game seed is {}", rebuilt_seed);
    }
  }

  let mut round: usize = 0;
  let mut assigned: usize = 1;
  while assigned > 0 && ! is_solved(&board) {
    round += 1;
    if verbose {
      println!("Round {}", round);
    }
    assigned = solve(&mut board, verbose);
  }

  if is_solved(&board) {
    println!("Finished solver, puzzle is solved.");
    print_board(&board);
    std::process::exit(0);
  }
  
  // Dynamic programming
  // Branch on cells with minimal number of free values
  if verbose {
    println!("Finished initial solver");
    print_board(&board);
    println!("Starting dynamic programming");
  }

  // TODO: move below into function
  // DFS with depth of branch
  // How to store trial board states? (unflattened, flattened, seed?)

  // Build priority queue (min-heap) of cells to branch on
  #[derive(Copy, Clone, Eq, PartialEq)]
  struct Branch {
    _pos: usize,
    _free: usize
  }
  impl Ord for Branch {
    fn cmp(&self, other: &Branch) -> Ordering {
      other._free.cmp(&self._free)
        .then_with(|| other._pos.cmp(&self._pos))
    }
  }
  impl PartialOrd for Branch {
    fn partial_cmp(&self, other: &Branch) -> Option<Ordering> {
        Some(self.cmp(other))
    }
  }
  let mut pq = BinaryHeap::new();
  for row in 0..9 {
    for col in 0..9 {
      if board[id(row, col)] != 0u8 {
        continue
      }
      let used = get_used(&board, id(row, col), BoardArea::ALL);
      let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
      pq.push(Branch {_pos: id(row, col), _free: free.len()});
    }
  }

  //
  while let Some(Branch {_pos, _free}) = pq.pop() {
    let used = get_used(&board, _pos, BoardArea::ALL);
    let free: BTreeSet<u8> = get_universe().difference(&used).cloned().collect();
    if verbose {
      println!("Position [{}] has {} free values: {:?}", _pos, _free, free);
    }
  }
}
