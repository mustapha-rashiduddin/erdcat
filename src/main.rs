use clap::Parser;
use erdcat::emit::{self, Format};
use erdcat::schema::Schema;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "erdcat",
    version,
    about = "Print a SQLite schema as an ER diagram"
)]
struct Args {
    db_path: PathBuf,
    #[arg(short, long, value_enum, default_value = "dot")]
    format: Format,
}

fn main() {
    let args = Args::parse();
    match Schema::open(&args.db_path) {
        Ok(schema) => print!("{}", emit::render(args.format, &schema)),
        Err(e) => {
            eprintln!("erdcat: {}: {e}", args.db_path.display());
            std::process::exit(1);
        }
    }
}
