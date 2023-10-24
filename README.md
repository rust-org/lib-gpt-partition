GptPartitionCursor implements the Read + Write + Seek + Debug.

It's used for backing up or restoring partition images, such as in embedded upgrades.

```rust
use gpt_partition::*;
use std::io::{Seek, Write};

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let path = "/dev/mmcblk0";

    // Write the new GPT table to /dev/mmcblk0
    write_dev_with_gpt_img(&mut std::fs::File::open("./gpt.img")?, path).unwrap();

    //
    // Optional, fix GPT header and backup area.
    // (cargo add gpt-partition -F gpt_header_fixup)
    //
    gpt_header_fixup("/dev/mmcblk0").unwrap();

    let mut pt_boot = GptPartitionCursor::new(path, "boot").unwrap();
    println!("{:#?}", pt_boot);

    // Read the boot partition to boot.bin
    let mut fout = std::io::BufWriter::new(std::fs::File::create("boot.bin")?);
    std::io::copy(&mut pt_boot, &mut fout).unwrap();

    // Write the boot.bin to /dev/mmcblk0p1
    let mut fin = std::io::BufReader::new(std::fs::File::open("boot.bin")?);
    pt_boot.seek(std::io::SeekFrom::Start(0)).unwrap();
    let size = std::io::copy(&mut fin, &mut pt_boot).unwrap();
    pt_boot.flush().unwrap();
    println!("{:#?}", size == pt_boot.size);

    //
    // Check:
    // hexdump -C /dev/mmcblk0p1 | head
    //

    Ok(())
}
```
