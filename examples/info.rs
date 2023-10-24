use gpt_partition::*;


fn main() ->  Result<(), std::io::Error> {
    println!("{}", part_name_to_path("/dev/mmcblk0", "rootfs")?);

    Ok(())
}
