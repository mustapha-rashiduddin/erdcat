use crate::layout::{self, Layout, NodeBox, line_row};
use crate::schema::Schema;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

type Point = (i64, i64);

pub fn render(schema: &Schema, ascii: bool) -> String {
    let layout = layout::compute(schema);
    let routes = route_edges(&layout);
    let mut grid = Grid::new(&layout, &routes);
    grid.draw_boxes(&layout);
    grid.draw_edges(&routes);
    grid.finish(ascii)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Dir {
    R,
    D,
    L,
    U,
}

impl Dir {
    fn delta(self) -> Point {
        match self {
            Dir::R => (1, 0),
            Dir::D => (0, 1),
            Dir::L => (-1, 0),
            Dir::U => (0, -1),
        }
    }

    fn bit(self) -> u8 {
        match self {
            Dir::U => 1,
            Dir::R => 2,
            Dir::D => 4,
            Dir::L => 8,
        }
    }

    fn axis(self) -> u8 {
        match self {
            Dir::R | Dir::L => 2 | 8,
            Dir::U | Dir::D => 1 | 4,
        }
    }
}

fn direction(a: Point, b: Point) -> Dir {
    match (b.0 - a.0, b.1 - a.1) {
        (1, 0) => Dir::R,
        (0, 1) => Dir::D,
        (-1, 0) => Dir::L,
        (0, -1) => Dir::U,
        _ => unreachable!("route contains non-adjacent points"),
    }
}

fn arrow_glyph(d: Dir) -> char {
    match d {
        Dir::R => '>',
        Dir::D => 'v',
        Dir::L => '<',
        Dir::U => '^',
    }
}

fn glyph_dirs(c: char) -> Option<u8> {
    match c {
        '─' => Some(2 | 8),
        '│' => Some(1 | 4),
        '┌' => Some(2 | 4),
        '┐' => Some(8 | 4),
        '└' => Some(2 | 1),
        '┘' => Some(8 | 1),
        '├' => Some(2 | 1 | 4),
        '┤' => Some(8 | 1 | 4),
        '┬' => Some(2 | 4 | 8),
        '┴' => Some(2 | 1 | 8),
        '┼' => Some(1 | 2 | 4 | 8),
        _ => None,
    }
}

fn dirs_glyph(m: u8) -> char {
    match m {
        0b0010 | 0b1000 | 0b1010 => '─',
        0b0001 | 0b0100 | 0b0101 => '│',
        0b0110 => '┌',
        0b1100 => '┐',
        0b0011 => '└',
        0b1001 => '┘',
        0b0111 => '├',
        0b1101 => '┤',
        0b1110 => '┬',
        0b1011 => '┴',
        0b1111 => '┼',
        _ => ' ',
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Debug, Copy, Clone)]
struct Port {
    border: Point,
    outside: Point,
}

fn port(node: &NodeBox, row: i64, side: Side) -> Port {
    match side {
        Side::Left => Port {
            border: (node.x, row),
            outside: (node.x - 1, row),
        },
        Side::Right => {
            let x = node.x + node.w as i64 - 1;
            Port {
                border: (x, row),
                outside: (x + 1, row),
            }
        }
    }
}

#[derive(Debug)]
struct RoutedEdge {
    source: Port,
    target: Port,
    path: Vec<Point>,
    bidirectional: bool,
}

#[derive(Debug, Copy, Clone)]
struct Bounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl Bounds {
    fn contains(self, p: Point) -> bool {
        p.0 >= self.min_x && p.0 <= self.max_x && p.1 >= self.min_y && p.1 <= self.max_y
    }
}

fn route_edges(layout: &Layout) -> Vec<RoutedEdge> {
    if layout.nodes.is_empty() {
        return Vec::new();
    }

    let mut blocked = HashSet::new();
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for node in &layout.nodes {
        let right = node.x + node.w as i64 - 1;
        let bottom = node.y + node.h as i64 - 1;
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
        for y in node.y..=bottom {
            for x in node.x..=right {
                blocked.insert((x, y));
            }
        }
    }

    let margin = 6 + layout.edges.len() as i64;
    let bounds = Bounds {
        min_x: min_x - margin,
        min_y: min_y - margin,
        max_x: max_x + margin,
        max_y: max_y + margin,
    };
    let mut used: HashMap<Point, u8> = HashMap::new();
    let mut port_use: HashMap<Point, usize> = HashMap::new();
    let mut routes = Vec::new();

    let mut pending: Vec<_> = layout.edges.iter().collect();
    pending.sort_by_key(|edge| !edge.bidirectional);
    for edge in pending {
        let Some(source_node) = layout.nodes.iter().find(|n| n.name == edge.from) else {
            continue;
        };
        let Some(target_node) = layout.nodes.iter().find(|n| n.name == edge.to) else {
            continue;
        };
        let source_row = line_row(source_node, edge.from_line);
        let target_row = line_row(target_node, edge.to_line);
        let source_center = source_node.x + source_node.w as i64 / 2;
        let target_center = target_node.x + target_node.w as i64 / 2;
        let source_sides = if target_center >= source_center {
            [Side::Right, Side::Left]
        } else {
            [Side::Left, Side::Right]
        };
        let target_sides = if target_center >= source_center {
            [Side::Left, Side::Right]
        } else {
            [Side::Right, Side::Left]
        };

        let mut best: Option<(usize, Port, Port, Vec<Point>)> = None;
        for source_side in source_sides {
            for target_side in target_sides {
                let source = port(source_node, source_row, source_side);
                let target = port(target_node, target_row, target_side);
                if source.outside == target.outside
                    || blocked.contains(&source.outside)
                    || blocked.contains(&target.outside)
                {
                    continue;
                }
                let Some((cost, path)) =
                    astar(source.outside, target.outside, bounds, &blocked, &used)
                else {
                    continue;
                };
                let score = cost
                    + port_use.get(&source.border).copied().unwrap_or(0) * 20
                    + port_use.get(&target.border).copied().unwrap_or(0) * 25;
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _, _, _)| score < *best_score)
                {
                    best = Some((score, source, target, path));
                }
            }
        }

        let Some((_, source, target, path)) = best else {
            continue;
        };
        record_usage(&mut used, source, target, &path);
        *port_use.entry(source.border).or_default() += 1;
        *port_use.entry(target.border).or_default() += 1;
        routes.push(RoutedEdge {
            source,
            target,
            path,
            bidirectional: edge.bidirectional,
        });
    }

    routes
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct SearchState {
    point: Point,
    dir: Option<Dir>,
}

fn astar(
    start: Point,
    goal: Point,
    bounds: Bounds,
    blocked: &HashSet<Point>,
    used: &HashMap<Point, u8>,
) -> Option<(usize, Vec<Point>)> {
    let start_state = SearchState {
        point: start,
        dir: None,
    };
    let mut open = BinaryHeap::new();
    let mut scores = HashMap::new();
    let mut previous = HashMap::new();
    let mut sequence = 0usize;
    scores.insert(start_state, 0usize);
    open.push(Reverse((
        manhattan(start, goal),
        0usize,
        sequence,
        start_state,
    )));

    while let Some(Reverse((_, score, _, state))) = open.pop() {
        if scores.get(&state).copied() != Some(score) {
            continue;
        }
        if state.point == goal {
            let mut path = vec![state.point];
            let mut current = state;
            while current != start_state {
                current = previous[&current];
                path.push(current.point);
            }
            path.reverse();
            return Some((score, path));
        }

        for dir in [Dir::R, Dir::D, Dir::L, Dir::U] {
            let delta = dir.delta();
            let next_point = (state.point.0 + delta.0, state.point.1 + delta.1);
            if !bounds.contains(next_point) || blocked.contains(&next_point) {
                continue;
            }
            let mut step = 1usize;
            if state.dir.is_some_and(|previous| previous != dir) {
                step += 4;
            }
            if let Some(mask) = used.get(&next_point) {
                if mask & dir.axis() != 0 {
                    step += 15;
                }
                if mask & !dir.axis() & 0b1111 != 0 {
                    step += 4;
                }
            }
            if next_point != goal && next_point != start && adjacent_to_blocked(next_point, blocked)
            {
                step += 1;
            }
            let next = SearchState {
                point: next_point,
                dir: Some(dir),
            };
            let next_score = score + step;
            if next_score >= scores.get(&next).copied().unwrap_or(usize::MAX) {
                continue;
            }
            scores.insert(next, next_score);
            previous.insert(next, state);
            sequence += 1;
            open.push(Reverse((
                next_score + manhattan(next_point, goal),
                next_score,
                sequence,
                next,
            )));
        }
    }

    None
}

fn manhattan(a: Point, b: Point) -> usize {
    (a.0.abs_diff(b.0) + a.1.abs_diff(b.1)) as usize
}

fn adjacent_to_blocked(p: Point, blocked: &HashSet<Point>) -> bool {
    [Dir::R, Dir::D, Dir::L, Dir::U].into_iter().any(|dir| {
        let delta = dir.delta();
        blocked.contains(&(p.0 + delta.0, p.1 + delta.1))
    })
}

fn route_mask(path: &[Point], i: usize, source: Port, target: Port) -> u8 {
    let previous = if i == 0 { source.border } else { path[i - 1] };
    let next = if i + 1 == path.len() {
        target.border
    } else {
        path[i + 1]
    };
    direction(path[i], previous).bit() | direction(path[i], next).bit()
}

fn record_usage(used: &mut HashMap<Point, u8>, source: Port, target: Port, path: &[Point]) {
    for (i, point) in path.iter().copied().enumerate() {
        *used.entry(point).or_default() |= route_mask(path, i, source, target);
    }
}

struct Grid {
    cells: Vec<Vec<char>>,
    off_x: i64,
    off_y: i64,
}

impl Grid {
    fn new(layout: &Layout, routes: &[RoutedEdge]) -> Self {
        if layout.nodes.is_empty() {
            return Grid {
                cells: Vec::new(),
                off_x: 0,
                off_y: 0,
            };
        }
        let mut min_x = i64::MAX;
        let mut min_y = i64::MAX;
        let mut max_x = i64::MIN;
        let mut max_y = i64::MIN;
        for node in &layout.nodes {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.w as i64 - 1);
            max_y = max_y.max(node.y + node.h as i64 - 1);
        }
        for route in routes {
            for point in route
                .path
                .iter()
                .chain([&route.source.border, &route.target.border])
            {
                min_x = min_x.min(point.0);
                min_y = min_y.min(point.1);
                max_x = max_x.max(point.0);
                max_y = max_y.max(point.1);
            }
        }
        Grid {
            cells: vec![vec![' '; (max_x - min_x + 3) as usize]; (max_y - min_y + 3) as usize],
            off_x: -min_x + 1,
            off_y: -min_y + 1,
        }
    }

    fn put(&mut self, point: Point, c: char) {
        let x = (point.0 + self.off_x) as usize;
        let y = (point.1 + self.off_y) as usize;
        if y < self.cells.len() && x < self.cells[y].len() {
            self.cells[y][x] = c;
        }
    }

    fn merge_mask(&mut self, point: Point, mask: u8) {
        let x = (point.0 + self.off_x) as usize;
        let y = (point.1 + self.off_y) as usize;
        if y >= self.cells.len() || x >= self.cells[y].len() {
            return;
        }
        let current = self.cells[y][x];
        if let Some(existing) = glyph_dirs(current) {
            self.cells[y][x] = dirs_glyph(existing | mask);
        } else if current == ' ' {
            self.cells[y][x] = dirs_glyph(mask);
        }
    }

    fn draw_boxes(&mut self, layout: &Layout) {
        for node in &layout.nodes {
            let right = node.x + node.w as i64 - 1;
            let bottom = node.y + node.h as i64 - 1;
            self.put((node.x, node.y), '┌');
            self.put((right, node.y), '┐');
            self.put((node.x, bottom), '└');
            self.put((right, bottom), '┘');
            for x in node.x + 1..right {
                self.put((x, node.y), '─');
                self.put((x, bottom), '─');
            }
            for y in node.y + 1..bottom {
                self.put((node.x, y), '│');
                self.put((right, y), '│');
            }
            if node.lines.len() > 1 {
                let y = node.y + 2;
                self.put((node.x, y), '├');
                self.put((right, y), '┤');
                for x in node.x + 1..right {
                    self.put((x, y), '─');
                }
            }
            for (index, line) in node.lines.iter().enumerate() {
                let y = line_row(node, index);
                let len = line.chars().count() as i64;
                let padding = (node.w as i64 - 2 - len).max(0) / 2;
                for (offset, c) in line.chars().enumerate() {
                    self.put((node.x + 1 + padding + offset as i64, y), c);
                }
            }
        }
    }

    fn draw_edges(&mut self, routes: &[RoutedEdge]) {
        for route in routes {
            self.merge_mask(
                route.source.border,
                direction(route.source.border, route.source.outside).bit(),
            );
            self.merge_mask(
                route.target.border,
                direction(route.target.border, route.target.outside).bit(),
            );
            for (i, point) in route.path.iter().copied().enumerate() {
                self.merge_mask(
                    point,
                    route_mask(&route.path, i, route.source, route.target),
                );
            }
        }
        for route in routes {
            self.put(
                route.target.outside,
                arrow_glyph(direction(route.target.outside, route.target.border)),
            );
            if route.bidirectional {
                self.put(
                    route.source.outside,
                    arrow_glyph(direction(route.source.outside, route.source.border)),
                );
            }
        }
    }

    fn finish(&self, ascii: bool) -> String {
        let Some(first) = self
            .cells
            .iter()
            .position(|row| row.iter().any(|c| *c != ' '))
        else {
            return String::new();
        };
        let last = self
            .cells
            .iter()
            .rposition(|row| row.iter().any(|c| *c != ' '))
            .unwrap();
        let mut out = String::new();
        for row in &self.cells[first..=last] {
            if let Some(end) = row.iter().rposition(|c| *c != ' ') {
                for c in &row[..=end] {
                    out.push(if ascii {
                        match c {
                            '─' => '-',
                            '│' => '|',
                            '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' => {
                                '+'
                            }
                            other => *other,
                        }
                    } else {
                        *c
                    });
                }
            }
            out.push('\n');
        }
        out
    }
}
