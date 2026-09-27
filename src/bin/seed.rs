#[tokio::main]
async fn main() {
    if let Err(error) = elsewhere::seed::run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("Catalog seed complete.");
}
