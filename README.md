# Ogg Magic

**Ogg Magic** is a Rust library designed to faithfully parse and edit metadata in Ogg Vorbis files according to the Vorbis and Ogg specifications. This library is intended for developers who need to debug and manipulate the structure of Ogg Vorbis files, rather than for efficient data reading.

## Features

- **Comprehensive Parsing**: Parses Ogg Vorbis file structures and metadata with high fidelity to the specifications.
- **Detailed Exposure**: Exposes all encoding-related information, allowing developers to inspect every detail of the file's encoding.
- **Editing Capabilities**: Provides limited editing functions to update metadata and comments within Ogg Vorbis files.
- **Utility Functions**: Includes utility functions for common file operations, such as trimming and collecting pages, which are particularly useful for repairing Ogg containers.
- **Transparency**: Almost no private methods, ensuring that all relevant information is accessible to developers.

### Utility Functions

- **Collect All Pages**: Collects all pages of an Ogg Vorbis file.
- **Find Packet by Type**: Finds a packet of a specified type in an Ogg Vorbis file.
- **Trim Ogg Vorbis File**: Trims an Ogg Vorbis file by removing segments before the first header and data after the last segment.

## Examples

Refer to the `examples` directory for more detailed usage examples.

## Examples

The following examples demonstrate how to perform key operations with the Ogg Magic library, such as trimming an Ogg Vorbis file and updating its metadata.

### Trimming an Ogg Vorbis File

This example shows how to trim an Ogg Vorbis file by removing unnecessary pages before the first header and after the last segment, to fix a broken container.

```rust
use tokio::fs::File;
use tokio::io::BufReader;
use ogg_magic::utils::trim_ogg_vorbis_file;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = "path/to/your/input.ogg";
    let output_path = "path/to/your/output.ogg";

    let input_file = File::open(input_path).await?;
    let mut reader = BufReader::new(input_file);

    // Perform trim operation
    let trimmed_pages = trim_ogg_vorbis_file(&mut reader, false, 3).await?;

    // Write the trimmed pages to the output file
    let mut output_file = File::create(output_path).await?;
    for page in trimmed_pages {
        output_file.write_all(&page.page.buffer).await?;
    }

    Ok(())
}
```

### Updating Metadata Comments

This example demonstrates how to update the metadata comments in an Ogg Vorbis file.

```rust
use tokio::fs::File;
use tokio::io::BufReader;
use ogg_magic::utils::{find_packet_by_type, update_ogg_vorbis_comments};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = "path/to/your/input.ogg";
    let output_path = "path/to/your/output.ogg";

    let input_file = File::open(input_path).await?;
    let mut reader = BufReader::new(input_file);

    // Perform trim operation
    let trimmed_pages = trim_ogg_vorbis_file(&mut reader, false, 3).await?;

    // Define new comments
    let mut new_comments = HashMap::new();
    new_comments.insert("TITLE".to_string(), vec!["New Title".to_string()]);
    new_comments.insert("ALBUM".to_string(), vec!["New Album".to_string()]);
    new_comments.insert("ARTIST".to_string(), vec!["New Artist".to_string()]);

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
        let mut output_file = File::create(output_path).await?;
        for page in updated_pages {
            output_file.write_all(&page.page.buffer).await?;
        }
    } else {
        eprintln!("No comment packet found in the Ogg Vorbis file.");
    }

    Ok(())
}
```

### Collecting All Pages

This example shows how to collect all pages of an Ogg Vorbis file.

```rust
use tokio::fs::File;
use tokio::io::BufReader;
use ogg_magic::utils::collect_ogg_vorbis_file;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = "path/to/your/input.ogg";

    let input_file = File::open(input_path).await?;
    let mut reader = BufReader::new(input_file);

    // Collect all pages
    let pages = collect_ogg_vorbis_file(&mut reader).await?;

    // Process pages as needed
    for page in pages {
        println!("Page: {:?}", page);
    }

    Ok(())
}
```

These examples cover some of the key operations you can perform with the Ogg Magic library. For more detailed examples and use cases, please refer to the `examples` directory in the repository.

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open an issue.

## License

This project is licensed under the MIT License.

---

For more information, please refer to the documentation and examples provided in the repository. Happy coding!