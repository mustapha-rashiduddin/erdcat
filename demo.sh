#!/usr/bin/env sh
# Build a demo SQLite db and print its ER diagram.
# Usage: ./demo.sh [unicode|ascii|dot|mermaid]
set -e
cd "$(dirname "$0")"
DB=/tmp/opencode/erdcat-demo.db
rm -f "$DB"
sqlite3 "$DB" <<'SQL'
CREATE TABLE author (
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
);
SQL
cargo --quiet run -- "$DB" --format "${1:-unicode}"
