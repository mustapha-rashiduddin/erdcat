use erdcat::emit::{self, Format};
use erdcat::layout;
use erdcat::schema::Schema;
use rusqlite::Connection;

fn schema(sql: &str) -> Schema {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(sql).unwrap();
    Schema::load(&conn).unwrap()
}

fn fixture() -> Schema {
    schema(
        "CREATE TABLE author (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE book (id INTEGER PRIMARY KEY, title TEXT, author_id INTEGER REFERENCES author(id));
         CREATE TABLE author_book (
             author_id INTEGER REFERENCES author(id),
             book_id INTEGER REFERENCES book(id),
             PRIMARY KEY (author_id, book_id)
         );
         CREATE TABLE lonely (x INTEGER);",
    )
}

fn stress_fixture() -> Schema {
    schema(
        "CREATE TABLE author (
             id INTEGER PRIMARY KEY,
             name TEXT NOT NULL,
             email TEXT
         );
         CREATE TABLE book (
             id INTEGER PRIMARY KEY,
             title TEXT NOT NULL,
             author_id INTEGER REFERENCES author(id),
             co_author_id INTEGER REFERENCES author(id)
         );
         CREATE TABLE author_book (
             author_id INTEGER REFERENCES author(id),
             book_id INTEGER REFERENCES book(id),
             PRIMARY KEY (author_id, book_id)
         );
         CREATE TABLE chapter (
             id INTEGER PRIMARY KEY,
             book_id INTEGER REFERENCES book(id),
             title TEXT
         );
         CREATE TABLE node (
             id INTEGER PRIMARY KEY,
             parent_id INTEGER REFERENCES node(id),
             title TEXT
         );",
    )
}

fn row<'a>(diagram: &'a str, text: &str) -> &'a str {
    diagram.lines().find(|line| line.contains(text)).unwrap()
}

#[test]
fn dot_renders_tables_edges_and_junctions() {
    let dot = emit::render(Format::Dot, &fixture());
    assert!(dot.starts_with("digraph erd {"));
    assert!(dot.contains("\"author\""));
    assert!(dot.contains("\"book\""));
    assert!(dot.contains("\"lonely\""));
    assert!(!dot.contains("\"author_book\""));
    assert!(dot.contains("\"book\":col_author_id -> \"author\":col_id"));
    assert!(dot.contains("arrowtail=crow"));
    let mn = dot.contains("\"book\":col_id -> \"author\":col_id")
        || dot.contains("\"author\":col_id -> \"book\":col_id");
    assert!(mn);
    assert!(dot.ends_with("}\n"));
}

#[test]
fn mermaid_renders_entities_and_relationships() {
    let mmd = emit::render(Format::Mermaid, &fixture());
    assert!(mmd.starts_with("erDiagram\n"));
    assert!(mmd.contains("author {"));
    assert!(mmd.contains("INTEGER id PK"));
    assert!(mmd.contains("author ||--o{ book"));
    assert!(!mmd.contains("author_book {"));
}

#[test]
fn self_reference_and_missing_target_resolve() {
    let schema = schema(
        "CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node);
         CREATE TABLE orphan (id INTEGER, other_id INTEGER REFERENCES missing);",
    );
    let dot = emit::render(Format::Dot, &schema);
    assert!(dot.contains("\"node\":col_parent_id -> \"node\":col_id"));
    assert!(dot.contains("\"orphan\":col_other_id -> \"missing\""));
    let unicode = emit::render(Format::Unicode, &schema);
    assert!(!row(&unicode, "missing").contains(['<', '>']));
    assert!(row(&unicode, "?").contains(['<', '>']));
}

#[test]
fn unicode_renders_boxes_columns_and_edges() {
    let out = emit::render(Format::Unicode, &fixture());
    assert!(out.contains('┌'));
    assert!(out.contains('├'));
    assert!(out.contains("│ author"));
    assert!(out.contains("id PK"));
    assert!(out.contains("title TEXT"));
    assert!(!out.contains("author_book"));
    assert!(row(&out, "author_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "id PK INTEGER").contains(['<', '>']));
    assert!(!row(&out, "author     ").contains(['<', '>']));
}

#[test]
fn unicode_simple_fk_matches_golden_grid() {
    let schema = schema(
        "CREATE TABLE author (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE book (
             id INTEGER PRIMARY KEY,
             title TEXT,
              author_id INTEGER REFERENCES author(id)
          );",
    );
    let expected = concat!(
        " ┌───────────────────┐\n",
        " │       book        │        ┌───────────────┐\n",
        " ├───────────────────┤        │    author     │\n",
        " │   id PK INTEGER   │        ├───────────────┤\n",
        " │    title TEXT     │       >┤ id PK INTEGER │\n",
        " │ author_id INTEGER ├───────┘│   name TEXT   │\n",
        " └───────────────────┘        └───────────────┘\n",
    );
    assert_eq!(emit::render(Format::Unicode, &schema), expected);
}

#[test]
fn ascii_variant_uses_plain_glyphs() {
    let out = emit::render(Format::Ascii, &fixture());
    assert!(out.contains('+'));
    assert!(out.contains('|'));
    assert!(!out.contains('─'));
}

#[test]
fn unicode_renders_self_reference_loop() {
    let schema =
        schema("CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node);");
    let out = emit::render(Format::Unicode, &schema);
    let expected = concat!(
        " ┌───────────────────┐\n",
        " │       node        │\n",
        " ├───────────────────┤\n",
        " │   id PK INTEGER   ├<\n",
        " │ parent_id INTEGER ├┘\n",
        " └───────────────────┘\n",
    );
    assert_eq!(out, expected);
    assert!(row(&out, "parent_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "id PK INTEGER").contains(['<', '>']));
    assert!(!row(&out, "node").contains(['<', '>']));
}

#[test]
fn parallel_and_junction_edges_use_column_ports() {
    let schema = stress_fixture();
    let out = emit::render(Format::Unicode, &schema);
    assert!(row(&out, "book_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "author_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "co_author_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "parent_id INTEGER").contains(['├', '┤']));
    assert!(
        layout::compute(&schema)
            .edges
            .iter()
            .all(|edge| edge.from_line > 0 && edge.to_line > 0)
    );
}

#[test]
fn print_unicode_diagram() {
    println!("{}", emit::render(Format::Unicode, &stress_fixture()));
}

#[test]
fn sqlite_metadata_is_normalized_without_changing_names() {
    let schema = schema(
        "CREATE TABLE \"Parent\" (
             \"A\" INTEGER,
             \"B\" INTEGER,
             total INTEGER GENERATED ALWAYS AS (\"A\" + \"B\") STORED,
             PRIMARY KEY (\"B\", \"A\")
         );
         CREATE TABLE \"Child\" (
             \"X\" INTEGER,
             \"Y\" INTEGER,
             FOREIGN KEY (\"x\", \"y\") REFERENCES \"parent\"
         );
         CREATE TABLE sqliteX (id INTEGER);
         CREATE VIRTUAL TABLE docs USING fts5(body);",
    );

    let parent = &schema.tables["Parent"];
    assert!(parent.columns.iter().any(|column| column.name == "total"));
    assert_eq!(parent.columns[0].primary_key_position, 2);
    assert_eq!(parent.columns[1].primary_key_position, 1);
    assert!(schema.tables.contains_key("sqliteX"));
    assert!(schema.tables.contains_key("docs"));
    assert!(!schema.tables.contains_key("docs_data"));

    let child = &schema.tables["Child"];
    let x = child
        .foreign_keys
        .iter()
        .find(|fk| fk.from_column == "X")
        .unwrap();
    let y = child
        .foreign_keys
        .iter()
        .find(|fk| fk.from_column == "Y")
        .unwrap();
    assert_eq!((x.to_table.as_str(), x.to_column.as_str()), ("Parent", "B"));
    assert_eq!((y.to_table.as_str(), y.to_column.as_str()), ("Parent", "A"));
}

#[test]
fn unsafe_junctions_remain_visible() {
    let missing_targets = schema(
        "CREATE TABLE broken_link (
             a INTEGER REFERENCES missing_a(id),
             b INTEGER REFERENCES missing_b(id),
             PRIMARY KEY (a, b)
         );",
    );
    assert!(missing_targets.collapsible_junctions().is_empty());
    let out = emit::render(Format::Unicode, &missing_targets);
    assert!(out.contains("broken_link"));
    assert!(out.contains("missing_a"));
    assert!(out.contains("missing_b"));

    let incoming_reference = schema(
        "CREATE TABLE author (id INTEGER PRIMARY KEY);
         CREATE TABLE book (id INTEGER PRIMARY KEY);
         CREATE TABLE author_book (
             author_id INTEGER REFERENCES author(id),
             book_id INTEGER REFERENCES book(id),
             PRIMARY KEY (author_id, book_id)
         );
         CREATE TABLE audit (
             author_id INTEGER,
             book_id INTEGER,
             FOREIGN KEY (author_id, book_id)
                 REFERENCES author_book(author_id, book_id)
         );",
    );
    assert!(
        !incoming_reference
            .collapsible_junctions()
            .contains("author_book")
    );
    assert!(emit::render(Format::Unicode, &incoming_reference).contains("author_book"));
}

#[test]
fn junction_edges_anchor_to_referenced_columns() {
    let schema = schema(
        "CREATE TABLE left_parent (id INTEGER PRIMARY KEY, code TEXT UNIQUE);
         CREATE TABLE right_parent (id INTEGER PRIMARY KEY, code TEXT UNIQUE);
         CREATE TABLE link (
             left_code TEXT REFERENCES left_parent(code),
             right_code TEXT REFERENCES right_parent(code),
             PRIMARY KEY (left_code, right_code)
         );",
    );
    let edge = layout::compute(&schema)
        .edges
        .into_iter()
        .find(|edge| edge.bidirectional)
        .unwrap();
    assert_eq!(edge.from_line, 2);
    assert_eq!(edge.to_line, 2);
}

#[test]
fn terminal_unsafe_names_are_escaped() {
    let schema = schema("CREATE TABLE \"wide界\" (\"line\nbreak\" TEXT);");
    let out = emit::render(Format::Unicode, &schema);
    assert!(out.contains("wide\\u{754c}"));
    assert!(out.contains("line\\nbreak TEXT"));
}
