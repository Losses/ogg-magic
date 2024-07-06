use clap::Parser;
use rand::Rng;
use std::collections::HashMap;
use tokio::fs::OpenOptions;
use tokio::io::{self, AsyncWriteExt, BufReader};

use librespot_ogg::utils::{find_packet_by_type, trim_ogg_vorbis_file, update_ogg_vorbis_comments};

/// A program to trim an Ogg Vorbis file and update specific fields with random values.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input Ogg Vorbis file
    #[clap(short, long)]
    input: String,

    /// Output Ogg Vorbis file
    #[clap(short, long)]
    output: String,
}

/// Main function to execute the trimming and updating of Ogg Vorbis file comments.
///
/// This function performs the following steps:
/// 1. Parses command line arguments to get input and output file paths.
/// 2. Opens the input Ogg Vorbis file.
/// 3. Trims the Ogg Vorbis file to remove unnecessary pages.
/// 4. Generates random values for the TITLE, ALBUM, PERFORMER, and ARTIST comments.
/// 5. Finds the comment packet in the trimmed Ogg Vorbis file.
/// 6. Updates the comments with the generated random values.
/// 7. Writes the updated Ogg Vorbis pages to the output file.
///
/// # Arguments
///
/// * `args` - Command line arguments specifying the input and output file paths.
///
/// # Errors
///
/// This function will return an error if any of the file operations (reading, writing) fail,
/// or if the Ogg Vorbis file does not contain a comment packet.
#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Open input file
    let input_file = tokio::fs::File::open(&args.input).await?;
    let mut reader = BufReader::new(input_file);

    // Perform trim operation
    let trimmed_pages = trim_ogg_vorbis_file(&mut reader, false, 3).await?;

    // Generate random values for the fields
    let mut rng = rand::thread_rng();
    let random_values: Vec<String> = (0..3)
        .map(|_| {
            (0..10)
                .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                .collect()
        })
        .collect();

    // Define new comments
    let mut new_comments = HashMap::new();
    new_comments.insert("TITLE".to_string(), vec![random_values[0].clone()]);
    new_comments.insert("ALBUM".to_string(), vec![random_values[1].clone()]);
    new_comments.insert("PERFORMER".to_string(), vec![random_values[2].clone()]);
    new_comments.insert("ARTIST".to_string(), vec![random_values[2].clone()]);

    // Find the comment packet
    if let Some((comments_page_index, comments_index)) =
        find_packet_by_type(&trimmed_pages, "comment")
    {
        // Update the comments
        let updated_pages = update_ogg_vorbis_comments(
            trimmed_pages,
            comments_page_index,
            comments_index,
            new_comments,
        );

        // Write the updated pages to the output file
        let mut output_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&args.output)
            .await?;
        for page in updated_pages {
            output_file.write_all(&page.page.buffer).await?;
        }
    } else {
        eprintln!("No comment packet found in the Ogg Vorbis file.");
    }

    Ok(())
}