use crate::schema::{Schema, Table};
use dagre::graph::{Graph, GraphOptions};
use dagre::{EdgeLabel, NodeLabel, RankDir, layout};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone)]
pub struct NodeBox {
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: usize,
    pub h: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EdgeRoute {
    pub from: String,
    pub to: String,
    pub from_line: usize,
    pub to_line: usize,
    pub bidirectional: bool,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<NodeBox>,
    pub edges: Vec<EdgeRoute>,
}

fn box_lines(name: &str, t: Option<&Table>) -> Vec<String> {
    let mut lines = vec![name.to_string()];
    match t {
        Some(t) => {
            for c in &t.columns {
                let mut s = c.name.clone();
                if c.primary_key {
                    s.push_str(" PK");
                }
                if !c.data_type.is_empty() {
                    s.push_str(&format!(" {}", c.data_type));
                }
                lines.push(s);
            }
        }
        None => lines.push("?".to_string()),
    }
    lines
}

fn node_size(lines: &[String]) -> (usize, usize) {
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4;
    let h = if lines.len() > 1 {
        lines.len() + 3
    } else {
        lines.len() + 2
    };
    (w.max(8), h)
}

pub fn compute(schema: &Schema) -> Layout {
    let junction_names: BTreeSet<&str> = schema
        .tables
        .values()
        .filter(|t| t.junction_targets().is_some())
        .map(|t| t.name.as_str())
        .collect();

    let mut names: BTreeSet<String> = schema
        .tables
        .keys()
        .filter(|n| !junction_names.contains(n.as_str()))
        .cloned()
        .collect();
    for t in schema.tables.values() {
        if junction_names.contains(t.name.as_str()) {
            continue;
        }
        for fk in &t.foreign_keys {
            if !junction_names.contains(fk.to_table.as_str()) {
                names.insert(fk.to_table.clone());
            }
        }
    }

    let mut edges = Vec::new();
    let mut sizes: HashMap<String, (usize, usize)> = HashMap::new();
    for name in &names {
        let lines = box_lines(name, schema.tables.get(name));
        sizes.insert(name.clone(), node_size(&lines));
    }

    for t in schema.tables.values() {
        if junction_names.contains(t.name.as_str()) {
            continue;
        }
        for fk in &t.foreign_keys {
            if junction_names.contains(fk.to_table.as_str()) {
                continue;
            }
            let fl = column_line(schema.tables.get(&t.name), &fk.from_column);
            let tl = column_line(schema.tables.get(&fk.to_table), &fk.to_column);
            edges.push(EdgeRoute {
                from: t.name.clone(),
                to: fk.to_table.clone(),
                from_line: fl,
                to_line: tl,
                bidirectional: false,
            });
        }
    }

    for t in schema.tables.values() {
        if let Some((a, b)) = t.junction_targets() {
            edges.push(EdgeRoute {
                from: a.clone(),
                to: b.clone(),
                from_line: pk_line(schema.tables.get(&a)),
                to_line: pk_line(schema.tables.get(&b)),
                bidirectional: true,
            });
        }
    }

    let connected: BTreeSet<String> = edges
        .iter()
        .flat_map(|edge| [edge.from.clone(), edge.to.clone()])
        .collect();
    let mut g = Graph::<NodeLabel, EdgeLabel>::with_options(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });
    for name in &connected {
        g.set_node(
            name.as_str(),
            Some(NodeLabel {
                width: sizes[name].0 as f64,
                height: sizes[name].1 as f64,
                ..Default::default()
            }),
        );
    }
    for edge in &edges {
        g.set_edge(
            edge.from.as_str(),
            edge.to.as_str(),
            Some(EdgeLabel::default()),
            None,
        );
    }

    let opts = dagre::LayoutOptions {
        rankdir: RankDir::LR,
        nodesep: 2.0,
        ranksep: 8.0,
        ..Default::default()
    };
    if !connected.is_empty() {
        layout(&mut g, Some(opts));
    }

    let mut ranks: BTreeMap<i32, Vec<NodeBox>> = BTreeMap::new();
    for name in g.nodes() {
        let label = g.node(&name).expect("laid-out node");
        let (w, h) = sizes[&name];
        let x = (label.x.unwrap_or(0.0) - w as f64 / 2.0).round() as i64;
        let y = (label.y.unwrap_or(0.0) - h as f64 / 2.0).round() as i64;
        ranks
            .entry(label.rank.unwrap_or(0))
            .or_default()
            .push(NodeBox {
                name: name.clone(),
                x,
                y,
                w,
                h,
                lines: box_lines(&name, schema.tables.get(&name)),
            });
    }

    let mut nodes = Vec::new();
    for rank in ranks.values_mut() {
        rank.sort_by_key(|node| node.y);
        if rank.len() > 1 {
            let top = rank.first().unwrap().y;
            let bottom = rank
                .iter()
                .map(|node| node.y + node.h as i64)
                .max()
                .unwrap();
            let packed_height =
                rank.iter().map(|node| node.h as i64).sum::<i64>() + 2 * (rank.len() as i64 - 1);
            let mut cursor = (top + bottom - packed_height) / 2;
            for node in rank.iter_mut() {
                node.y = cursor;
                cursor += node.h as i64 + 2;
            }
        }
        nodes.append(rank);
    }

    let mut x = 0i64;
    let mut y = nodes
        .iter()
        .map(|node| node.y + node.h as i64)
        .max()
        .map_or(0, |bottom| bottom + 3);
    let mut row_height = 0i64;
    for name in names.iter().filter(|name| !connected.contains(*name)) {
        let (w, h) = sizes[name];
        if x > 0 && x + w as i64 > 100 {
            x = 0;
            y += row_height + 2;
            row_height = 0;
        }
        nodes.push(NodeBox {
            name: name.clone(),
            x,
            y,
            w,
            h,
            lines: box_lines(name, schema.tables.get(name)),
        });
        x += w as i64 + 4;
        row_height = row_height.max(h as i64);
    }

    Layout { nodes, edges }
}

fn column_line(t: Option<&Table>, col: &str) -> usize {
    t.and_then(|t| t.columns.iter().position(|c| c.name == col))
        .map_or(1, |i| i + 1)
}

fn pk_line(t: Option<&Table>) -> usize {
    t.and_then(|t| t.columns.iter().position(|c| c.primary_key))
        .map_or(1, |i| i + 1)
}

pub fn line_row(n: &NodeBox, idx: usize) -> i64 {
    if idx == 0 {
        n.y + 1
    } else {
        n.y + 2 + idx as i64
    }
}
