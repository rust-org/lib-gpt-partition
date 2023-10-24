//#![feature(rustc_private)]
use log::*;
use std::os::unix::io::AsRawFd;
use std::io::*;


fn get_sector_size(f:&mut std::fs::File) -> usize{
    let fd = f.as_raw_fd();
    let mut sector_size: libc::c_int = 0;
    unsafe {
        if libc::ioctl(fd, libc::BLKSSZGET, &mut sector_size) != 0 {
            return 512;
        }
    }
    trace!("get_sector_size: {}", sector_size);
    if sector_size != 512 && sector_size != 4096 {
        return 512;
        }
    return sector_size as usize;
    }


trait ReadTrait {
    fn read_vec(&mut self, size: isize) -> core::result::Result<Vec<u8>, std::io::Error>;
    }

impl<R: std::io::Read> ReadTrait for R {
    fn read_vec(&mut self, size: isize) -> core::result::Result<Vec<u8>, std::io::Error> {
        let mut vec:Vec<u8> = Vec::new();
        if size < 0 {
            self.read_to_end(&mut vec)?;
            }
        else if size > 0 {
            vec = Vec::with_capacity(size as usize);
            unsafe {vec.set_len(size as usize);}
            let mut buf = &mut vec[..];
            let mut readed = 0;
            while !buf.is_empty() {
                match self.read(buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        readed += n;
                        let tmp = buf;
                        buf = &mut tmp[n..];
                        }
                    Err(e) => return Err(e),
                    }
                }
            unsafe {vec.set_len(readed);}
            }
        return Ok(vec);
        }
    }

macro_rules! write {
    ($dst:expr, $src:expr) => {
        let val = $src;
        let dst_ptr = $dst as *mut _ as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(
                &val as *const _ as *const u8,
                dst_ptr,
                std::mem::size_of_val(&val),
            );
        }
    };
}

use crc32fast;
use std::ops::Add;
use std::ops::Sub;
use std::ops::Div;

fn div_ceil<T>(x: T, y: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + From<u8> + Div<Output = T>,
{
    (x + y - T::from(1u8)) / y
}


pub fn gpt_header_fixup(dev_path :&str) -> std::io::Result<()>{
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(dev_path)?;

    f.seek(SeekFrom::Start(0x200))?; // skip MBR

    //---------------------------------------------------------
    //                check gpt header magic
    //---------------------------------------------------------
    let mut gpt_header = f.read_vec(0x5c)?;
    if &gpt_header[0..8] != b"EFI PART" || gpt_header.len() != 0x5c {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid GPT Header"));
        }

    //---------------------------------------------------------
    //                get sector size
    //---------------------------------------------------------
    let sector_size = get_sector_size(&mut f) as u64;
    trace!("sector_size: {}", sector_size);

    //---------------------------------------------------------
    //                get base info
    //---------------------------------------------------------
    // 128
    let part_count = u32::from_le_bytes(gpt_header[0x50..][..4].try_into().unwrap());
    // 128
    let part_item_size = u32::from_le_bytes(gpt_header[0x54..][..4].try_into().unwrap());
    // 128 * 128
    let part_table_size = (part_count * part_item_size) as u64;

    let gpt_start = u64::from_le_bytes(gpt_header[0x18..][..8].try_into().unwrap()) * sector_size;
    let part_table_start = u64::from_le_bytes(gpt_header[0x48..][..8].try_into().unwrap()) * sector_size;
    //let parts_start = u64::from_le_bytes(gpt_header[0x28..][..8].try_into().unwrap()) * sector_size;

    let gpt_end = part_table_start + part_table_size;
    let gpt_size = gpt_end - sector_size;

    trace!("gpt_start: {}", gpt_start);
    trace!("part_table_start: {}", part_table_start);
    trace!("part_table_size: {}", part_table_size);
    trace!("gpt_size: {}", gpt_size);
    trace!("gpt_end: {}", gpt_end);

    //---------------------------------------------------------
    //                fix part table crc32
    //---------------------------------------------------------
    f.seek(SeekFrom::Start(part_table_start))?;
    let table = f.read_vec(part_table_size as isize)?;
    let mut crc32 = crc32fast::Hasher::new();
    crc32.update(&table[..]);
    let table_crc32 = crc32.finalize();
    trace!("Part Table CRC32: {:#x} -> {:#x}", u32::from_le_bytes(gpt_header[0x58..][..4].try_into().unwrap()), table_crc32);
    write!(&mut gpt_header[0x58], table_crc32);

    //---------------------------------------------------------
    //                fix gpt size
    //---------------------------------------------------------
    //let h2_lba = find_backup_gpt_header_lba(&mut f, sector_size)?;
    let disk_end_lba = f.seek(std::io::SeekFrom::End(0))? / sector_size - 1;
    trace!("backup_gpt_header_lba: {} -> {}", u64::from_le_bytes(gpt_header[0x20..][..8].try_into().unwrap()),disk_end_lba);
    write!(&mut gpt_header[0x20], disk_end_lba);
    // 34
    let parts_start_lba = div_ceil(gpt_end, sector_size) as u64;
    let parts_end_lba = f.seek(std::io::SeekFrom::End(-(parts_start_lba as i64) * sector_size as i64))? / sector_size;
    // let end_of_disk_lba = h2_lba;
    trace!("size_of_parts_lba: [{}:{}] -> [{}:{}]", u64::from_le_bytes(gpt_header[0x28..][..8].try_into().unwrap()), u64::from_le_bytes(gpt_header[0x30..][..8].try_into().unwrap()), parts_start_lba, parts_end_lba);
    write!(&mut gpt_header[0x28], parts_start_lba);
    write!(&mut gpt_header[0x30], parts_end_lba);

    //---------------------------------------------------------
    //                fix gpt header crc32 and write
    //---------------------------------------------------------
    let header_crc32_ori = u32::from_le_bytes(gpt_header[16..][..4].try_into().unwrap());
    write!(&mut gpt_header[0x10], 0u32);
    write!(&mut gpt_header[0x18], 1u64);
    let mut crc32 = crc32fast::Hasher::new();
    crc32.update(&gpt_header[..]);
    let actual_crc = crc32.finalize();
    trace!("Header CRC32: {:#x} -> {:#x}", header_crc32_ori, actual_crc);
    write!(&mut gpt_header[0x10], actual_crc);
    f.seek(SeekFrom::Start(0x200))?; // skip MBR
    f.write_all(&gpt_header)?;

    //---------------------------------------------------------
    //                fix backup gpt header and write
    //---------------------------------------------------------
    write!(&mut gpt_header[0x10], 0u32);
    write!(&mut gpt_header[0x18], disk_end_lba);
    write!(&mut gpt_header[0x20], 1);
    let mut crc32 = crc32fast::Hasher::new();
    crc32.update(&gpt_header[..]);
    let actual_crc = crc32.finalize();
    trace!("Backup Header CRC32: {:#x}", actual_crc);
    write!(&mut gpt_header[0x10], actual_crc);
    f.seek(SeekFrom::End(-1*sector_size as i64))?;
    f.write_all(&gpt_header)?;

    //---------------------------------------------------------
    //                build backup part table
    //---------------------------------------------------------
    f.seek(SeekFrom::Start(part_table_start))?;
    let part_table = f.read_vec(part_table_size as isize)?;
    if part_table.len() != part_table_size as usize {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid GPT Partition Table"));
        }
    f.seek(SeekFrom::Start((parts_end_lba+1)*sector_size))?;
    f.write_all(&part_table)?;

    //---------------------------------------------------------
    //                build pmbr
    //---------------------------------------------------------
    let mut pmbr = [0; 512];

    //
    // 0x1be: 00 | 00 02 00 | ee | ff ff ff | 01 00 00 00 | ff ff ff ff
    // echo -ne '\x00\x00\x02\x00\xee\xff\xff\xff\x01\x00\x00\x00\xff\xff\xff\xff' | dd of=/dev/mmcblk0 bs=1 seek=$((0x1be)) conv=notrunc
    // https://wiki.osdev.org/GPT
    //

    pmbr[0x1be..][..16].copy_from_slice(&[0x00, 0x00, 0x02, 0x00, 0xee, 0xff, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff]);
    if disk_end_lba < 0xFFFFFFFF {
        pmbr[0x1be + 12..][..4].copy_from_slice(& (disk_end_lba as u32).to_le_bytes());
        }
    pmbr[510..][..2].copy_from_slice(&[0x55, 0xaa]);

    f.seek(SeekFrom::Start(0))?;
    f.write_all(&pmbr)?;

    f.flush()?;
    Ok(())
    }
