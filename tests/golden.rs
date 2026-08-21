use erdcat::emit::{self, Format};
use erdcat::schema::Schema;
use rusqlite::Connection;

fn fixture() -> Schema {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE author (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE book (id INTEGER PRIMARY KEY, title TEXT, author_id INTEGER REFERENCES author(id));
         CREATE TABLE author_book (
             author_id INTEGER REFERENCES author(id),
             book_id INTEGER REFERENCES book(id),
             PRIMARY KEY (author_id, book_id)
         );
         CREATE TABLE lonely (x INTEGER);",
    )
    .unwrap();
    Schema::load(&conn).unwrap()
}

fn stress_fixture() -> Schema {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
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
    .unwrap();
    Schema::load(&conn).unwrap()
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
    let mn = dot.contains("\"book\" -> \"author\"") || dot.contains("\"author\" -> \"book\"");
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
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node);
         CREATE TABLE orphan (id INTEGER, other_id INTEGER REFERENCES missing);",
    )
    .unwrap();
    let schema = Schema::load(&conn).unwrap();
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
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE author (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE book (
             id INTEGER PRIMARY KEY,
             title TEXT,
             author_id INTEGER REFERENCES author(id)
         );",
    )
    .unwrap();
    let schema = Schema::load(&conn).unwrap();
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
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node);",
    )
    .unwrap();
    let schema = Schema::load(&conn).unwrap();
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
    let out = emit::render(Format::Unicode, &stress_fixture());
    assert!(row(&out, "book_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "author_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "co_author_id INTEGER").contains(['├', '┤']));
    assert!(row(&out, "parent_id INTEGER").contains(['├', '┤']));
    assert!(!row(&out, "chapter     ").contains(['<', '>']));
    assert!(!row(&out, "book         ").contains(['<', '>']));
    assert!(!row(&out, "author     ").contains(['<', '>']));
    assert!(!row(&out, "node        ").contains(['<', '>']));
}

#[test]
fn print_unicode_diagram() {
    println!("{}", emit::render(Format::Unicode, &stress_fixture()));
}
